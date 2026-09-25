# Records docs/demo/dashboard.gif. Run from the repository root: vhs docs/demo/dashboard.tape
Output docs/demo/dashboard.gif

Set Shell "bash"
Set FontSize 14
Set Width 1280
Set Height 760
Set Padding 16
Set Framerate 24
Set Theme "Dracula"
Set WindowBar Colorful
Set TypingSpeed 70ms

Hide
Type "cd docs/demo && (bun apps.ts & bash traffic.sh &) > /dev/null 2>&1 && clear"
Enter
Sleep 1s
Show

Type "bunf 5174 3000"
Sleep 500ms
Enter
Sleep 7s

# DNS takes 10 to 40 seconds: skip the rest of the digging.
Hide
Wait+Screen@120s /any key for the dashboard/
Show
Sleep 4s

Sleep 12s
Type "f"
Sleep 900ms
Type "f"
Sleep 2s
Type "r"
Sleep 3s
Escape
Sleep 1500ms
Type "q"
Sleep 3500ms
