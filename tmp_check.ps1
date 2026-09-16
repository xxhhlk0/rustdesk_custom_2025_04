$L = 'C:\Windows\ServiceProfiles\LocalService\AppData\Roaming\RustDesk\log\server'
Write-Output '=== DLL CHECK ==='
Get-ChildItem 'C:\Program Files\RustDesk\librustdesk.dll' | ForEach-Object { $_.LastWriteTime }
Write-Output '=== LOG FILES ==='
$latest = Get-ChildItem $L -File | Sort-Object LastWriteTime -Descending | Select-Object -First 3
foreach ($f in $latest) {
  Write-Output ('--- ' + $f.Name + ' (' + $f.Length + ' bytes, ' + $f.LastWriteTime + ')')
}
Write-Output '=== ENC STATS (newest file only) ==='
$f = $latest | Select-Object -First 1
Get-Content $f.FullName | Select-String -Pattern 'video enc stats' | ForEach-Object { $_.Line }
Write-Output '=== ENCODER EVENTS (newest file only) ==='
Get-Content $f.FullName | Select-String -Pattern 'new encoder|hw encode params|hw encode opened|qsv rate control|used preference|disable vram|changed to gdi|fall back' | ForEach-Object {
  $line = $_.Line
  if ($line.Length -gt 200) { $line = $line.Substring(0, 200) }
  $line
}
