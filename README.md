# palmrest

The Magic Mouse gesture daemon of the desktop shell program: reads the mouse's touch reports
(hidraw), turns them into scroll and gesture streams with the scroll "feel" of the reference
desktop, and delivers them over a socket to shell-host surfaces and, as a virtual touchpad, to
every other client. Specified in quire's `design/12-BEHAVIOUR-gestures.md` (and `design/11`
for how the host uses the stream). Portable to any Linux; no shell dependency.
