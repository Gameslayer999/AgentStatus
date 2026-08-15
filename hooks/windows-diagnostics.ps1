# AgentStatus — Windows diagnostics (decisions 068/072).
#
# Three checks that cannot be answered by reading code or window styles, each of which
# caught a real defect during the Windows port:
#
#   -Consoles   Watch for *visible* console windows. Counting conhost.exe processes proves
#               nothing — CREATE_NO_WINDOW still creates a console device — so only
#               enumerating visible windows distinguishes "a console was allocated" from
#               "a console blinked on the user's screen". Found the bar's 10-second
#               `claude agents --json` poll flashing a window all day.
#   -Tray       Ask the shell's own UI tree whether AgentStatus has a taskbar button (it
#               must not: `skipTaskbar`) and whether it has a notification-area icon (it
#               must, in tray mode). tao implements skipTaskbar via ITaskbarList::DeleteTab,
#               which leaves no trace in WS_EX_*, so the window styles cannot answer this.
#   -Windows    List every top-level window the app owns, with visibility. Tells "the panel
#               is hidden and a tray helper exists" apart from "the app died" — which
#               MainWindowHandle alone will mislead you about.
#
# Reports whether AgentStatus is present; it does not print other applications' names.
#
#   pwsh hooks/windows-diagnostics.ps1 -Consoles -Seconds 30
#   pwsh hooks/windows-diagnostics.ps1 -Tray
#   pwsh hooks/windows-diagnostics.ps1 -Windows
param(
  [switch]$Consoles,
  [switch]$Tray,
  [switch]$Windows,
  [int]$Seconds = 30
)
if (-not ($Consoles -or $Tray -or $Windows)) { $Consoles = $Tray = $Windows = $true }

Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public class ASDiag {
  delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }

  static readonly string[] ConsoleClasses = { "ConsoleWindowClass", "PseudoConsoleWindow" };

  public static List<string> WatchConsoles(int seconds) {
    var hits = new List<string>();
    var seen = new HashSet<IntPtr>();
    var end = DateTime.Now.AddSeconds(seconds);
    while (DateTime.Now < end) {
      EnumWindows((h, l) => {
        if (!IsWindowVisible(h) || seen.Contains(h)) return true;
        var cls = new StringBuilder(256); GetClassNameW(h, cls, cls.Capacity);
        string c = cls.ToString();
        bool isConsole = false;
        foreach (var k in ConsoleClasses) if (c == k) isConsole = true;
        if (!isConsole) return true;
        seen.Add(h);
        uint pid; GetWindowThreadProcessId(h, out pid);
        string name = "?";
        try { name = System.Diagnostics.Process.GetProcessById((int)pid).ProcessName; } catch {}
        hits.Add(String.Format("{0:HH:mm:ss.fff}  class={1} pid={2} proc={3}", DateTime.Now, c, pid, name));
        return true;
      }, IntPtr.Zero);
      System.Threading.Thread.Sleep(25);
    }
    return hits;
  }

  public static List<string> WindowsForPid(uint want) {
    var outp = new List<string>();
    EnumWindows((h, l) => {
      uint pid; GetWindowThreadProcessId(h, out pid);
      if (pid != want) return true;
      var c = new StringBuilder(256); GetClassNameW(h, c, c.Capacity);
      var t = new StringBuilder(256); GetWindowTextW(h, t, t.Capacity);
      RECT r; GetWindowRect(h, out r);
      outp.Add(String.Format("visible={0,-5} class={1,-30} {2}x{3}@({4},{5}) title='{6}'",
        IsWindowVisible(h), c.ToString(), r.Right-r.Left, r.Bottom-r.Top, r.Left, r.Top, t.ToString()));
      return true;
    }, IntPtr.Zero);
    return outp;
  }
}
"@

if ($Windows) {
  Write-Output "== app windows =="
  $proc = Get-Process app -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $proc) { Write-Output "  app.exe is not running" }
  else { [ASDiag]::WindowsForPid([uint32]$proc.Id) | ForEach-Object { Write-Output "  $_" } }
}

if ($Tray) {
  Write-Output "== taskbar / notification area =="
  Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
  $root = [System.Windows.Automation.AutomationElement]::RootElement
  $btnCond = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
    [System.Windows.Automation.ControlType]::Button)

  function Report([System.Windows.Automation.AutomationElement]$Host_, [string]$What) {
    if (-not $Host_) { Write-Output "  ${What}: not found"; return }
    $names = @()
    foreach ($b in $Host_.FindAll([System.Windows.Automation.TreeScope]::Descendants, $btnCond)) {
      if ($b.Current.Name) { $names += $b.Current.Name }
    }
    $hit = @($names | Where-Object { $_ -like '*AgentStatus*' })
    Write-Output ("  {0}: {1} entries; AgentStatus present = {2}" -f $What, $names.Count, ($hit.Count -gt 0))
  }

  # NOT $tray: PowerShell variables are case-insensitive, so that would clobber the -Tray
  # switch parameter and the script would argue with itself.
  $trayRoot = $root.FindFirst([System.Windows.Automation.TreeScope]::Children,
    (New-Object System.Windows.Automation.PropertyCondition(
      [System.Windows.Automation.AutomationElement]::ClassNameProperty, 'Shell_TrayWnd')))
  Report $trayRoot 'taskbar + visible tray (must be False: the bar sets skipTaskbar)'

  # Windows 11 files new notification icons into the hidden-icons flyout, so an icon that is
  # working correctly will be found here rather than on the taskbar.
  $chev = $null
  if ($trayRoot) {
    foreach ($b in $trayRoot.FindAll([System.Windows.Automation.TreeScope]::Descendants, $btnCond)) {
      if ($b.Current.Name -match 'hidden icons|Show Hidden') { $chev = $b; break }
    }
  }
  if ($chev) {
    $chev.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    Start-Sleep -Milliseconds 1200
    $ov = $root.FindFirst([System.Windows.Automation.TreeScope]::Children,
      (New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ClassNameProperty, 'TopLevelWindowForOverflowXamlIsland')))
    Report $ov 'hidden-icons flyout (True while in tray mode)'
    Add-Type -AssemblyName System.Windows.Forms
    [System.Windows.Forms.SendKeys]::SendWait("{ESC}")
  } else { Write-Output "  hidden-icons flyout: no chevron (all icons shown)" }
}

if ($Consoles) {
  Write-Output "== visible console windows over $Seconds s =="
  Write-Output "   (nothing from app.exe / claude.exe / bash.exe should appear)"
  $hits = [ASDiag]::WatchConsoles($Seconds)
  if ($hits.Count -eq 0) { Write-Output "  none" }
  else { $hits | ForEach-Object { Write-Output "  $_" } }
}
