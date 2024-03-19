Output tapes/demo.gif

Require cargo

Set Shell "bash"
Set Theme "Dracula"
Set FontSize 28
Set Width 1800
Set Height 1200

Type@100ms "kubectl get pod -n kube-system kube-apiserver-kind-control-plane -ojson | jnv" Enter Sleep 2s
Type ".m" Sleep 1s
Tab 1 Sleep 2s
Down@2s 2
Up@2s 2
Enter Sleep 2s
Down@1s 6
Down@2s 3
Enter Sleep 2s
Type ".n" Sleep 1s
Tab@1s 3
Enter Sleep 3s
Ctrl+C
