// SPDX-License-Identifier: MIT
// Dedicated setup: a fixed USB allowlist, inbox WinUSB, and libwdi package signing.
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <shellapi.h>
#include <winhttp.h>
#include <bcrypt.h>
#include <filesystem>
#include <fstream>
#include <sstream>
#include <set>
#include <algorithm>
#include <cstdio>
#include <vector>
#include <stdexcept>
#include "libwdi.h"
#include "camera_setup_policy.hpp"

namespace fs = std::filesystem;
static std::string log_text;
static void progress(const std::string& text) {
    log_text += text + "\n";
    // QProcess supplies a stdout pipe even for this windowed executable.
    std::string line = text + "\n";
    DWORD written = 0;
    WriteFile(GetStdHandle(STD_OUTPUT_HANDLE), line.data(), DWORD(line.size()), &written, nullptr);
}
static std::wstring wide(const std::string& s) {
    if (s.empty()) return {};
    int size = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, s.data(), int(s.size()), nullptr, 0);
    if (!size) throw std::runtime_error("Invalid UTF-8 setup path");
    std::wstring result(size, 0);
    MultiByteToWideChar(CP_UTF8, 0, s.data(), int(s.size()), result.data(), size);
    return result;
}
static std::string utf8(const std::wstring& s) {
    int size = WideCharToMultiByte(CP_UTF8, 0, s.data(), int(s.size()), nullptr, 0, nullptr, nullptr);
    std::string result(size, 0);
    WideCharToMultiByte(CP_UTF8, 0, s.data(), int(s.size()), result.data(), size, nullptr, nullptr);
    return result;
}
static fs::path data_directory() {
    wchar_t value[32768];
    DWORD size = GetEnvironmentVariableW(L"LOCALAPPDATA", value, 32768);
    if (!size || size >= 32768) throw std::runtime_error("LOCALAPPDATA unavailable");
    fs::path result = fs::path(value) / L"KohdaLab Camera";
    fs::create_directories(result);
    return result;
}
static void check(int code, const char* operation) {
    if (code != WDI_SUCCESS) throw std::runtime_error(std::string(operation) + ": " + wdi_strerror(code));
}
struct DeviceList {
    wdi_device_info* head = nullptr;
    DeviceList() {
        wdi_options_create_list options{};
        options.list_all = TRUE;
        options.list_hubs = FALSE;
        options.trim_whitespaces = TRUE;
        int code = wdi_create_list(&head, &options);
        if (code != WDI_ERROR_NO_DEVICE) check(code, "USB discovery");
    }
    ~DeviceList() { if (head) wdi_destroy_list(head); }
};
static const char* name(wdi_device_info* device) {
    return setup_device_name(device->vid, device->pid, device->is_composite,
        device->mi, device->compatible_id ? device->compatible_id : "");
}
static bool ready(wdi_device_info* device) {
    if (!device->driver || _stricmp(device->driver, "WinUSB") || !device->device_id) return false;
    std::wstring key = L"SYSTEM\\CurrentControlSet\\Enum\\" + wide(device->device_id) + L"\\Device Parameters";
    wchar_t value[4096]{}; DWORD bytes = sizeof(value);
    auto flags = RRF_RT_REG_SZ | RRF_RT_REG_MULTI_SZ;
    LONG code = RegGetValueW(HKEY_LOCAL_MACHINE, key.c_str(), L"DeviceInterfaceGUIDs", flags, nullptr, value, &bytes);
    if (code != ERROR_SUCCESS) {
        bytes = sizeof(value);
        code = RegGetValueW(HKEY_LOCAL_MACHINE, key.c_str(), L"DeviceInterfaceGUID", flags, nullptr, value, &bytes);
    }
    GUID guid{};
    return code == ERROR_SUCCESS && value[0] && CLSIDFromString(value, &guid) == S_OK;
}
static bool administrator() {
    SID_IDENTIFIER_AUTHORITY authority = SECURITY_NT_AUTHORITY;
    PSID group = nullptr; BOOL member = FALSE;
    if (!AllocateAndInitializeSid(&authority, 2, SECURITY_BUILTIN_DOMAIN_RID,
        DOMAIN_ALIAS_RID_ADMINS, 0,0,0,0,0,0, &group)) return false;
    CheckTokenMembership(nullptr, group, &member); FreeSid(group);
    return member;
}
static std::string inf_name(wdi_device_info* device) {
    char value[64]; std::snprintf(value, sizeof(value), "kcamera-%04x-%04x-%02x.inf", device->vid, device->pid, device->mi);
    return value;
}
static void prepare(wdi_device_info* device, const fs::path& directory) {
    wdi_options_prepare_driver options{};
    options.driver_type = WDI_WINUSB;
    char vendor[] = "KohdaLab";
    options.vendor_name = vendor;
    // Use an ASCII description from the allowlist, never device-provided INF text.
    wdi_device_info target = *device;
    target.desc = const_cast<char*>(name(device) ? name(device) : "KohdaLab Camera");
    check(wdi_prepare_driver(&target, utf8(directory.wstring()).c_str(), inf_name(device).c_str(), &options), "WinUSB package preparation");
}
static void backup_driver(wdi_device_info* device, const fs::path& directory) {
    std::wstring key=L"SYSTEM\\CurrentControlSet\\Enum\\"+wide(device->device_id);
    wchar_t driver_key[4096]{}; DWORD bytes=sizeof(driver_key);
    LONG code=RegGetValueW(HKEY_LOCAL_MACHINE,key.c_str(),L"Driver",RRF_RT_REG_SZ,nullptr,driver_key,&bytes);
    if (code==ERROR_FILE_NOT_FOUND) return;
    if (code!=ERROR_SUCCESS) throw std::runtime_error("Cannot inspect previous camera driver for backup");
    key=L"SYSTEM\\CurrentControlSet\\Control\\Class\\"+std::wstring(driver_key);
    wchar_t inf[512]{}; bytes=sizeof(inf);
    code=RegGetValueW(HKEY_LOCAL_MACHINE,key.c_str(),L"InfPath",RRF_RT_REG_SZ,nullptr,inf,&bytes);
    if (code!=ERROR_SUCCESS) throw std::runtime_error("Cannot locate previous driver INF for backup");
    std::wstring filename(inf);
    if (filename.rfind(L"oem",0)!=0) return; // Inbox drivers remain available from Windows.
    if (filename.size()<8 || filename.substr(filename.size()-4)!=L".inf" ||
        filename.find_first_not_of(L"0123456789",3)!=filename.size()-4)
        throw std::runtime_error("Unexpected previous driver INF filename");
    auto destination=directory/L"driver-backup";
    fs::create_directories(destination);
    wchar_t system[MAX_PATH]; GetSystemDirectoryW(system,MAX_PATH);
    auto program=fs::path(system)/L"pnputil.exe";
    std::wstring command=L"\""+program.wstring()+L"\" /export-driver \""+filename+L"\" \""+destination.wstring()+L"\"";
    std::vector<wchar_t> writable(command.begin(),command.end());writable.push_back(0);
    STARTUPINFOW startup{};startup.cb=sizeof(startup);PROCESS_INFORMATION process{};
    if (!CreateProcessW(program.c_str(),writable.data(),nullptr,nullptr,FALSE,CREATE_NO_WINDOW,
        nullptr,nullptr,&startup,&process)) throw std::runtime_error("Cannot start previous driver backup");
    DWORD wait=WaitForSingleObject(process.hProcess,60000),result=1;
    if (wait==WAIT_OBJECT_0) GetExitCodeProcess(process.hProcess,&result);
    CloseHandle(process.hThread);CloseHandle(process.hProcess);
    if (wait!=WAIT_OBJECT_0 || result!=0) throw std::runtime_error("Previous driver backup failed; camera driver left unchanged");
    progress("Previous OEM driver exported to "+utf8(destination.wstring()));
    std::ofstream(directory/L"previous-driver.txt",std::ios::app)
        << device->device_id << "\n" << utf8(filename) << "\n" << utf8(destination.wstring()) << "\n";
}
static int elevate(bool quiet) {
    wchar_t executable[32768]; GetModuleFileNameW(nullptr, executable, 32768);
    SHELLEXECUTEINFOW info{}; info.cbSize = sizeof(info); info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = L"runas"; info.lpFile = executable;
    info.lpParameters = quiet ? L"--install --ensure" : L"--install"; info.nShow = SW_SHOWNORMAL;
    if (!ShellExecuteExW(&info)) return 1;
    WaitForSingleObject(info.hProcess, INFINITE); DWORD code = 1;
    GetExitCodeProcess(info.hProcess, &code); CloseHandle(info.hProcess); return int(code);
}
struct SetupLock {
    HANDLE handle;
    SetupLock() : handle(CreateMutexW(nullptr, FALSE, L"Global\\KohdaLabCameraUSBSetup")) {
        if (!handle) throw std::runtime_error("Cannot lock USB setup");
        DWORD result = WaitForSingleObject(handle, 0);
        if (result != WAIT_OBJECT_0 && result != WAIT_ABANDONED) {
            CloseHandle(handle);
            throw std::runtime_error("Another USB setup is still running; wait for it to finish");
        }
    }
    ~SetupLock() { ReleaseMutex(handle); CloseHandle(handle); }
};
int WINAPI wWinMain(HINSTANCE, HINSTANCE, PWSTR arguments, int) {
    bool quiet = std::wstring(arguments).find(L"--ensure") != std::wstring::npos;
    if (std::wstring(arguments) == L"--self-test") {
        return setup_device_name(0x0547,0x4d33,false,0,"") &&
            !setup_device_name(0x3923,0x7618,false,0,"") &&
            !setup_device_name(0x0547,0x4d33,true,1,"") &&
            wdi_is_driver_supported(WDI_WINUSB, nullptr) &&
            wdi_is_file_embedded(nullptr, "installer_x64.exe") ? 0 : 1;
    }
    try {
        DeviceList devices;
        wdi_device_info* target = nullptr;
        for (auto* device=devices.head; device; device=device->next) {
            if (!name(device)) continue;
            if (target) throw std::runtime_error("Connect only one 0547:4D33 camera for setup");
            target=device;
        }
        if (!target) throw std::runtime_error("No 0547:4D33 camera connected");
        if (ready(target)) { progress("Camera WinUSB is ready"); return 0; }
        if (!administrator()) return elevate(quiet);
        SetupLock lock;
        auto directory=data_directory();
        auto drivers=directory / L"usb-drivers";
        fs::create_directories(drivers);
        progress(std::string("Assigning inbox WinUSB to ") + target->device_id +
            "; previous service=" + (target->driver ? target->driver : "none"));
        backup_driver(target,directory);
        wdi_set_log_level(WDI_LOG_LEVEL_WARNING);
        prepare(target,drivers);
        wdi_options_install_driver options{};
        options.pending_install_timeout=60000;
        check(wdi_install_driver(target,utf8(drivers.wstring()).c_str(),inf_name(target).c_str(),&options),"WinUSB assignment");
        DeviceList refreshed;
        bool verified=false;
        for (auto* device=refreshed.head;device;device=device->next) {
            if (device->device_id && target->device_id && !strcmp(device->device_id,target->device_id))
                verified=ready(device);
        }
        if (!verified) throw std::runtime_error("WinUSB not active; reconnect the camera and retry");
        progress("Camera WinUSB setup completed");
        std::ofstream(directory / L"usb-setup.log",std::ios::app) << log_text;
        return 0;
    } catch (const std::exception& error) {
        progress(std::string("Setup failed: ")+error.what());
        try { std::ofstream(data_directory()/L"usb-setup.log",std::ios::app)<<log_text; } catch (...) {}
        if (!quiet) MessageBoxW(nullptr,wide(log_text).c_str(),L"KohdaLab Camera USB Setup",MB_OK|MB_ICONERROR);
        return 1;
    }
}
