//go:build !windows

package zumen

import "os/exec"

// hideWindow は、Windows のほかでは何もしない。
func hideWindow(*exec.Cmd) {}
