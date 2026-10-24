# Run from the repository root after `cargo build --locked`:
#   vhs tapes/demo.tape
# Clean dark background noise, reduce the palette, then optimize similar frames:
#   ffmpeg -i tapes/demo.gif -filter_complex '[0:v]fps=12,format=rgba,colorkey=0x000000:0.2:0[fg];color=c=0x1e1f29:s=1000x680:r=12[bg];[bg][fg]overlay=shortest=1,split[a][b];[a]palettegen=max_colors=64:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle' -loop 0 tapes/demo.palette.gif
#   magick tapes/demo.palette.gif -fuzz 3% -layers Optimize tapes/demo.gif
#   rm tapes/demo.palette.gif
Output tapes/demo.gif

Require cat
Require ./target/debug/jnv

Set Shell "bash"
Set Theme "Dracula"
Set FontSize 20
Set Width 1000
Set Height 680
Set Padding 20
Set Framerate 12
Set CursorBlink false
Set TypingSpeed 45ms

# Use this checkout and its default config, independent of the user's settings.
Hide
Type `export PATH="$PWD/target/debug:$PATH"`
Enter
Type "unset NO_COLOR"
Enter
Type "clear"
Enter
Sleep 500ms
Show

Type "cat tapes/demo.json | jnv -c default.toml"
Enter
Sleep 1500ms

# Complete a key, then choose between matching nested paths.
Type ".m"
Tab
Sleep 1200ms
Enter
Type ".na"
Tab
Sleep 1s
Down
Sleep 1s
Up
Sleep 1s
Enter
Sleep 1s

# Apply a jq projection to extract a compact, useful result.
Ctrl+U
Type@35ms ".spec.containers[] | {name, image}"
Sleep 3s

# Relaunch with a custom prompt, JSON colors, indentation, and completion style.
Ctrl+C
Hide
Type "clear"
Enter
Sleep 300ms
Show
Type "# Custom style"
Enter
Type "jnv -c tapes/demo.toml tapes/demo.json"
Enter
Sleep 1500ms
Type ".metadata.na"
Tab
Sleep 1200ms
Down
Sleep 1200ms
Enter
Sleep 1s
Ctrl+U
Type@35ms ".spec.containers[] | {name, image}"
Sleep 3s

Hide
Ctrl+C
