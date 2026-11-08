# VHS Tape: agy-suite interactive terminal control deck
# Output configuration
Output assets/demo.gif
Output assets/demo.mp4

# Terminal window setup
Set FontSize 14
Set FontFamily "JetBrainsMono Nerd Font Mono"
Set LetterSpacing 0
Set LineHeight 1.15
Set Width 1200
Set Height 720
Set Padding 20
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set TypingSpeed 30ms

# Boot environment
Hide
Type "export TERM=xterm-256color" Enter
Type "export PATH=$HOME/projects/rust/agy-suite/target/release:$PATH" Enter
Type "cd $HOME/projects/rust/agy-suite" Enter
Type "clear" Enter
Show

# 1. Launch agy-ctrl control deck
Sleep 200ms
Type "agy-ctrl"
Sleep 200ms
Enter

# 2. Inspect Fleet Matrix & Agent Profiles
Sleep 600ms
Down
Sleep 350ms
Down
Sleep 350ms
Down
Sleep 500ms

# 3. Toggle tool permissions in the MCP matrix
Right
Sleep 400ms
Down
Sleep 300ms
Space
Sleep 400ms
Space
Sleep 400ms
Left
Sleep 350ms

# 4. Switch to Tool Catalog inspector
Tab
Sleep 700ms
Down
Sleep 300ms
Down
Sleep 300ms

# 5. Open integrated manual & telemetry
Type "?"
Sleep 1.5s
Escape
Sleep 400ms

# 6. Exit cleanly
Type "q"
Sleep 400ms
