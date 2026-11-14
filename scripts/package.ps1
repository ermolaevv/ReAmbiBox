# PowerShell Package Script for ReAmbiBox
param(
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Split-Path -Parent $ScriptDir
$DistDir = Join-Path $RepoRoot "target\dist"
$PortableDir = Join-Path $DistDir "ReAmbiBox-Portable"
$Version = "1.0.1"

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "   ReAmbiBox v$Version — Сборка и упаковка дистрибутива    " -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

# 1. Компиляция релизной версии
if (-not $SkipBuild) {
    Write-Host "`n>>> [1/4] Компиляция релизной сборки (cargo build --release)..." -ForegroundColor Yellow
    # Завершаем старый запущенный процесс, если он удерживает бинарник
    Stop-Process -Name "reambibox", "reambibox-app" -Force -ErrorAction SilentlyContinue
    Push-Location $RepoRoot
    try {
        $cargoCmd = if (Get-Command cargo -ErrorAction SilentlyContinue) { "cargo" } else { "$env:USERPROFILE\.cargo\bin\cargo.exe" }
        & $cargoCmd build --release --bin reambibox
        if ($LASTEXITCODE -ne 0) {
            throw "Ошибка компиляции Cargo"
        }
    } finally {
        Pop-Location
    }
}

$ExeSource = Join-Path $RepoRoot "target\release\reambibox.exe"
if (-not (Test-Path $ExeSource)) {
    throw "Исполняемый файл $ExeSource не найден!"
}

# 2. Подготовка каталога портативной версии
Write-Host "`n>>> [2/4] Подготовка файлов портативного релиза..." -ForegroundColor Yellow
Stop-Process -Name "reambibox", "reambibox-app" -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 250
if (Test-Path $PortableDir) {
    Remove-Item -Recurse -Force $PortableDir
}
New-Item -ItemType Directory -Force -Path $PortableDir | Out-Null

Copy-Item -Force $ExeSource (Join-Path $PortableDir "reambibox.exe")
# Иконка уже вшита в сам reambibox.exe (PE-ресурс IDI_ICON1), поэтому app.ico отдельным файлом не требуется

# Создаем краткое руководство README.txt
$ReadmeLines = @(
    "============================================================",
    "   ReAmbiBox v$Version - High-Performance Ambilight Service",
    "============================================================",
    "",
    "БЫСТРЫЙ СТАРТ:",
    "1. Запустите 'reambibox.exe'.",
    "   Программа свернется в системный трей (значок возле часов).",
    "2. Откройте панель управления в любом браузере: http://localhost:3637",
    "",
    "ГЛОБАЛЬНЫЕ ГОРЯЧИЕ КЛАВИШИ WINDOWS:",
    "- Настраиваются в панели управления (Web UI)",
    "- По умолчанию:",
    "  * Ctrl + Shift + A        - Включение / выключение подсветки",
    "  * Ctrl + Shift + Вверх    - Увеличение яркости (+10%)",
    "  * Ctrl + Shift + Вниз     - Уменьшение яркости (-10%)",
    "  * Ctrl + Shift + O        - Экранный оверлей зон калибровки",
    "  * Esc                     - Сохранить и закрыть оверлей",
    "",
    "ПАРАМЕТРЫ ЗАПУСКА:",
    "- reambibox.exe --minimized   - тихий запуск в трей без открытия окон",
    "",
    "УНИКАЛЬНОСТЬ ПРОЦЕССА:",
    "Новый запуск reambibox.exe автоматически завершает и замещает старый",
    "экземпляр, освобождая COM-порт без конфликтов."
)
$ReadmeLines | Out-File -FilePath (Join-Path $PortableDir "README.txt") -Encoding UTF8

# 3. Создание ZIP-архива
Write-Host "`n>>> [3/4] Создание архива ReAmbiBox-Portable-v$Version.zip..." -ForegroundColor Yellow
$ZipPath = Join-Path $DistDir "ReAmbiBox-Portable-v$Version.zip"
if (Test-Path $ZipPath) {
    Remove-Item -Force $ZipPath
}
Compress-Archive -Path "$PortableDir\*" -DestinationPath $ZipPath -Force
$ZipSize = (Get-Item $ZipPath).Length / 1MB
Write-Host ("  -> Создан архив: $ZipPath ({0:N2} MB)" -f $ZipSize) -ForegroundColor Green

# 4. Проверка и сборка установщика Inno Setup
Write-Host "`n>>> [4/4] Проверка компилятора инсталлятора Inno Setup..." -ForegroundColor Yellow
$IsccPaths = @(
    "ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles}\Inno Setup 6\ISCC.exe",
    "${env:LOCALAPPDATA}\Programs\Inno Setup 6\ISCC.exe"
)

$Iscc = $null
foreach ($p in $IsccPaths) {
    if (Get-Command $p -ErrorAction SilentlyContinue) {
        $Iscc = $p
        break
    } elseif (Test-Path $p) {
        $Iscc = $p
        break
    }
}

if ($Iscc) {
    Write-Host "  -> Компиляция Windows инсталлятора через $Iscc..." -ForegroundColor Cyan
    Stop-Process -Name "ReAmbiBox-Setup*", "*Setup*.tmp" -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 300
    $IssScript = Join-Path $ScriptDir "installer.iss"
    & $Iscc $IssScript
    if ($LASTEXITCODE -eq 0) {
        $SetupPath = Join-Path $DistDir "ReAmbiBox-Setup-v$Version.exe"
        if (Test-Path $SetupPath) {
            $SetupSize = (Get-Item $SetupPath).Length / 1MB
            Write-Host ("  -> Успешно создан инсталлятор: $SetupPath ({0:N2} MB)" -f $SetupSize) -ForegroundColor Green
        }
    } else {
        Write-Warning "Inno Setup завершился с предупреждением или ошибкой"
    }
} else {
    Write-Host "  [Инфо] Inno Setup 6 не обнаружен на машине. Портативный ZIP-релиз полностью готов!" -ForegroundColor Gray
    Write-Host "         Для сборки 'ReAmbiBox-Setup.exe' установите Inno Setup 6: https://jrsoftware.org/isdl.php" -ForegroundColor Gray
}

Write-Host "`n============================================================" -ForegroundColor Cyan
Write-Host "   Готово! Артефакты дистрибутива собраны в:               " -ForegroundColor Cyan
Write-Host "   $DistDir" -ForegroundColor Cyan
Write-Host "============================================================`n" -ForegroundColor Cyan
