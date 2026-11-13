param(
    [Parameter(Mandatory=$true)][string]$PayloadDirectory,
    [Parameter(Mandatory=$true)][string]$OutputJson,
    [switch]$RequireSigned
)
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
$root=(Resolve-Path -LiteralPath $PayloadDirectory).Path
if(Test-Path -LiteralPath $OutputJson){throw 'Report path already exists'}
$files=@(Get-ChildItem -LiteralPath $root -File -Recurse|Where-Object{$_.Extension -in @('.exe','.dll')})
if(!$files.Count){throw 'No executable files to audit'}
$records=@(foreach($f in $files){
    $signature=Get-AuthenticodeSignature -LiteralPath $f.FullName
    [pscustomobject]@{
        file=$f.FullName.Substring($root.Length).TrimStart('\')
        sha256=(Get-FileHash -LiteralPath $f.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        status=[string]$signature.Status
        signer_subject=if($signature.SignerCertificate){$signature.SignerCertificate.Subject}else{$null}
        signer_thumbprint=if($signature.SignerCertificate){$signature.SignerCertificate.Thumbprint}else{$null}
        timestamp_present=($null -ne $signature.TimeStamperCertificate)
        zone_identifier_present=($null -ne (Get-Item -LiteralPath $f.FullName -Stream Zone.Identifier -ErrorAction SilentlyContinue))
    }
})
$counts=@{};foreach($store in @('Cert:\CurrentUser\My','Cert:\LocalMachine\My')){
    $counts[$store]=@(Get-ChildItem $store -ErrorAction SilentlyContinue|Where-Object{$_.HasPrivateKey -and ($_.EnhancedKeyUsageList.ObjectId -contains '1.3.6.1.5.5.7.3.3')}).Count
}
$report=[ordered]@{schema_version=1;utc=[DateTime]::UtcNow.ToString('o');files=$records;all_signatures_valid=(@($records|Where-Object{$_.status -ne 'Valid'}).Count -eq 0);code_signing_private_key_certificate_counts=$counts;note='Read-only audit. A valid signature does not guarantee SmartScreen reputation or absence of enterprise blocking.'}
$report|ConvertTo-Json -Depth 5|Out-File -LiteralPath $OutputJson -Encoding utf8
if($RequireSigned -and !$report.all_signatures_valid){throw 'Signed release gate failed; inspect the saved report'}
