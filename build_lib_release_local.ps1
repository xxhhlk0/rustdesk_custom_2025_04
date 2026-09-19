. 'D:\BuildTools\devcmd.ps1'
# 本地构建用 LLVM 15 的 libclang: bindgen 0.65.1 不认识 clang 23 的新调用约定,
# 会把 aom/vpx config 结构体退化为 opaque(只有 _address)。LLVM15 与 CI(LLVM 15.0.6) 一致。
# 见 libs/scrap 的 bindgen 绑定修复说明。
$env:PATH = "$env:USERPROFILE\.cargo\bin;D:\LLVM15\bin;C:\Program Files\Python312\Scripts;D:\vcpkg\downloads\tools\cmake-3.30.1-windows\cmake-3.30.1-windows-i386\bin;" + $env:PATH
$env:LIBCLANG_PATH = 'D:\LLVM15\bin'
$env:VCPKG_ROOT = 'D:\vcpkg'
Set-Location 'D:\T\OpenCode\github-repos\rustdesk_custom_2025_04'
Write-Output ('env: LIBCLANG_PATH=' + $env:LIBCLANG_PATH + ' VCPKG_ROOT=' + $env:VCPKG_ROOT)
cargo build --lib --release -p rustdesk 2>&1
Write-Output ('BUILD_EXIT=' + $LASTEXITCODE)
if ($LASTEXITCODE -eq 0) { Get-ChildItem target\release\librustdesk.dll | ForEach-Object { Write-Output ('ARTIFACT=' + $_.FullName + ' size=' + $_.Length + ' time=' + $_.LastWriteTime) } }