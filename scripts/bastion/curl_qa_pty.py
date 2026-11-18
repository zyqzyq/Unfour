"""Shared Git Bash / ConPTY test terminal for synthetic loopback QA."""
import ctypes as c
from ctypes import wintypes as w
import os
from pathlib import Path
import threading
import time

K = c.WinDLL('kernel32', use_last_error=True)

class Coord(c.Structure):
    _fields_ = [('x', c.c_short), ('y', c.c_short)]


class Startup(c.Structure):
    _fields_ = [('cb', w.DWORD), ('reserved', w.LPWSTR), ('desktop', w.LPWSTR), ('title', w.LPWSTR),
                ('x', w.DWORD), ('y', w.DWORD), ('xs', w.DWORD), ('ys', w.DWORD),
                ('xc', w.DWORD), ('yc', w.DWORD), ('fill', w.DWORD), ('flags', w.DWORD),
                ('show', w.WORD), ('cbReserved2', w.WORD), ('reserved2', c.c_void_p),
                ('stdin', w.HANDLE), ('stdout', w.HANDLE), ('stderr', w.HANDLE)]


class StartupEx(c.Structure):
    _fields_ = [('info', Startup), ('attributes', c.c_void_p)]


class Process(c.Structure):
    _fields_ = [('process', w.HANDLE), ('thread', w.HANDLE), ('pid', w.DWORD), ('tid', w.DWORD)]


K.CreatePipe.argtypes = [c.POINTER(w.HANDLE), c.POINTER(w.HANDLE), c.c_void_p, w.DWORD]
K.CreatePseudoConsole.argtypes = [Coord, w.HANDLE, w.HANDLE, w.DWORD, c.POINTER(w.HANDLE)]
K.CreatePseudoConsole.restype = c.c_long
K.InitializeProcThreadAttributeList.argtypes = [c.c_void_p, w.DWORD, w.DWORD, c.POINTER(c.c_size_t)]
K.UpdateProcThreadAttribute.argtypes = [c.c_void_p, w.DWORD, c.c_size_t, c.c_void_p, c.c_size_t, c.c_void_p, c.c_void_p]
K.DeleteProcThreadAttributeList.argtypes = [c.c_void_p]
K.CreateProcessW.argtypes = [w.LPCWSTR, w.LPWSTR, c.c_void_p, c.c_void_p, w.BOOL, w.DWORD, c.c_void_p, w.LPCWSTR, c.c_void_p, c.POINTER(Process)]
K.ReadFile.argtypes = [w.HANDLE, c.c_void_p, w.DWORD, c.POINTER(w.DWORD), c.c_void_p]
K.WriteFile.argtypes = [w.HANDLE, c.c_void_p, w.DWORD, c.POINTER(w.DWORD), c.c_void_p]
K.CloseHandle.argtypes = [w.HANDLE]
K.ClosePseudoConsole.argtypes = [w.HANDLE]
K.TerminateProcess.argtypes = [w.HANDLE, w.UINT]
K.WaitForSingleObject.argtypes = [w.HANDLE, w.DWORD]


def shell_path(path):
    value = path.resolve().as_posix()
    return '/' + value[0].lower() + value[2:]


def quoted(path):
    return "'" + shell_path(path).replace("'", "'\\''") + "'"


class Terminal:
    def __init__(self, history, git, root):
        self.history = history
        ri, wi, ro, wo = w.HANDLE(), w.HANDLE(), w.HANDLE(), w.HANDLE()
        assert K.CreatePipe(c.byref(ri), c.byref(wi), None, 0)
        assert K.CreatePipe(c.byref(ro), c.byref(wo), None, 0)
        self.console = w.HANDLE()
        assert K.CreatePseudoConsole(Coord(180, 40), ri, wo, 0, c.byref(self.console)) == 0
        K.CloseHandle(ri); K.CloseHandle(wo)
        size = c.c_size_t()
        K.InitializeProcThreadAttributeList(None, 1, 0, c.byref(size))
        self.attrs = c.create_string_buffer(size.value)
        assert K.InitializeProcThreadAttributeList(self.attrs, 1, 0, c.byref(size))
        assert K.UpdateProcThreadAttribute(self.attrs, 0, 0x00020016, self.console, c.sizeof(w.HANDLE), None, None)
        startup = StartupEx(); startup.info.cb = c.sizeof(startup); startup.attributes = c.addressof(self.attrs)
        self.process = Process()
        env = dict(os.environ, PATH=str(git / 'usr/bin') + ';' + os.environ['PATH'], TERM='dumb',
                   HISTFILE=shell_path(history), HISTCONTROL='', HISTSIZE='100', PS1='QA_PARENT> ')
        env_block = c.create_unicode_buffer('\0'.join(key + '=' + value for key, value in sorted(env.items())) + '\0\0')
        # The native launcher must replace inherited redirected std handles
        # with this ConPTY's console handles before starting the MSYS process.
        command = c.create_unicode_buffer(
            f'"{Path(os.environ["SystemRoot"]) / "System32/cmd.exe"}" /d /s /c '
            f'""{git / "usr/bin/winpty.exe"}" -Xallow-non-tty '
            f'"{git / "usr/bin/bash.exe"}" --noprofile --norc -i < CONIN$ > CONOUT$ 2>&1"')
        assert K.CreateProcessW(None, command, None, None, False, 0x00080000 | 0x400,
                                env_block, str(root), c.byref(startup), c.byref(self.process)), c.get_last_error()
        self.output = bytearray(); self.input = wi; self.reader = ro
        def read():
            buf = c.create_string_buffer(4096); count = w.DWORD()
            while K.ReadFile(ro, buf, len(buf), c.byref(count), None) and count.value:
                data = buf.raw[:count.value]; self.output.extend(data)
                if b'\x1b[6n' in data:
                    self.write(b'\x1b[1;1R')
        threading.Thread(target=read, daemon=True).start()

    def write(self, data):
        count = w.DWORD()
        assert K.WriteFile(self.input, data, len(data), c.byref(count), None)
        assert count.value == len(data)

    def close(self):
        K.TerminateProcess(self.process.process, 0)
        K.ClosePseudoConsole(self.console)
        K.DeleteProcThreadAttributeList(self.attrs)
        for handle in [self.input, self.reader, self.process.process, self.process.thread]:
            K.CloseHandle(handle)


def wait(check, description, seconds=10):
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        if check():
            return
        time.sleep(.02)
    raise AssertionError(description)


def content(path):
    return path.read_bytes() if path.exists() else b''


def frame(value):
    return b'\x1e' + value.encode() + b'\x1f'
