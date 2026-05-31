# ell_http

ell_http is a debugging tool for recording WinHttp calls. All you have to do is build it and inject the DLL into the target project's import address table using a tool like CFF Explorer. ell_http will automatically search the program's main module for calls to WinHttp and log them.
