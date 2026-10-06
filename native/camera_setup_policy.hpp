// SPDX-License-Identifier: MIT
#pragma once
#include <string>
inline const char* setup_device_name(unsigned vid,unsigned pid,bool composite,
                                    unsigned interface_number,const std::string&) {
    if (composite && interface_number != 0) return nullptr;
    return vid==0x0547 && pid==0x4d33 ? "KohdaLab 3.1M USB Camera" : nullptr;
}
