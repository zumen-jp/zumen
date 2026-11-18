package zumen

import (
	"os/exec"
	"syscall"
)

// createNoWindow は、Windows の CREATE_NO_WINDOW。コンソールのない親（GUI のアプリ）から起こしても、窓を開かない。
const createNoWindow = 0x08000000

func hideWindow(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{HideWindow: true, CreationFlags: createNoWindow}
}
