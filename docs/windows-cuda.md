# Windows 11 / RTX 5060 Ti CUDA

The pinned `ort` 2.0.0-rc.13 downloads ONNX Runtime **1.28** with CUDA **13** support. On this workstation the default bundled CUDA provider registered but its first Gather kernel failed with `cudaErrorNoKernelImageForDevice` on the RTX 5060 Ti (`sm_120`). Merely installing CUDA DLLs did **not** repair that binary. Microsoft's official `onnxruntime-gpu` **1.28.0 Windows wheel** supplies a compatible CUDA provider; the wheel is a ZIP archive containing native DLLs and does **not** require a Python interpreter to run this Rust program. Keep `onnxruntime.dll` and both provider DLLs from the **same** 1.28.0 package.

For the tested setup, install NVIDIA driver 617.14 or another driver compatible with CUDA 13.4 and the RTX 5060 Ti, plus the Microsoft VC++ Redistributable x64. Obtain cuBLAS 13.4 and ORT 1.28.0 only from [NVIDIA's official redistributable](https://developer.download.nvidia.com/compute/cuda/redist/libcublas/windows-x86_64/) and [Microsoft's PyPI release](https://pypi.org/project/onnxruntime-gpu/1.28.0/). Other models/ops may additionally need matching CUDA runtime, cuDNN 9, cuFFT and cuRAND libraries per [ORT's CUDA requirements](https://onnxruntime.ai/docs/execution-providers/CUDA-ExecutionProvider.html#requirements); the multilingual graph was exercised with the cuBLAS archive alone.

The following PowerShell workflow uses a *pinned official CPython 3.14 wheel solely as a DLL archive*; its extracted DLLs are native and are independent of the host's Python version. Download and verify the archive before copying DLLs alongside the Rust executable:

```powershell
cargo build --release
New-Item -ItemType Directory -Force .\gpu-runtime | Out-Null
$wheel = ".\gpu-runtime\onnxruntime_gpu-1.28.0-cp314-cp314-win_amd64.whl"
Invoke-WebRequest "https://files.pythonhosted.org/packages/1f/8f/7d0613468fda5c15c53095f971abe934145d174c025f7fc98649a9454a64/onnxruntime_gpu-1.28.0-cp314-cp314-win_amd64.whl" -OutFile $wheel
if ((Get-FileHash $wheel -Algorithm SHA256).Hash.ToLowerInvariant() -ne "965f2cad344510bfe2a024ba5c485528caafcc3cf11b218dfb988b0f0e34057d") { throw "ORT GPU wheel hash mismatch" }
Copy-Item $wheel .\gpu-runtime\ort.zip
Expand-Archive .\gpu-runtime\ort.zip -DestinationPath .\gpu-runtime\ort
Copy-Item .\gpu-runtime\ort\onnxruntime\capi\onnxruntime.dll .\target\release\
Copy-Item .\gpu-runtime\ort\onnxruntime\capi\onnxruntime_providers_cuda.dll .\target\release\
Copy-Item .\gpu-runtime\ort\onnxruntime\capi\onnxruntime_providers_shared.dll .\target\release\
$cublas = ".\gpu-runtime\libcublas-windows-x86_64-13.4.1.3-archive.zip"
Invoke-WebRequest "https://developer.download.nvidia.com/compute/cuda/redist/libcublas/windows-x86_64/libcublas-windows-x86_64-13.4.1.3-archive.zip" -OutFile $cublas
Expand-Archive $cublas -DestinationPath .\gpu-runtime\cublas
$env:PATH = "$(Resolve-Path .\gpu-runtime\cublas\libcublas-windows-x86_64-13.4.1.3-archive\bin\x64);$env:PATH"
nvidia-smi
.\target\release\laya.exe --device cuda bool --text "Cancel my account" --question "Does the user want to cancel?"
.\target\release\laya.exe --device cuda bench --repetitions 5
```

The CUDA probe executes a real decision graph at startup, not just provider registration. Explicit `--device cuda` exits nonzero if a provider cannot initialize **or** its kernels cannot execute. `--device auto` reports a CPU fallback on stderr when that probe fails; `--device cpu` never tries CUDA. `laya-inspect` shows graph metadata and which EPs were compiled, not a guarantee of successful GPU execution. Standard output of inference remains JSON. Do not place `gpu-runtime` in version control.

Tested on RTX 5060 Ti with NVIDIA driver 617.14: the stock Pyke CUDA provider failed at Gather with `cudaErrorNoKernelImageForDevice`; replacing the three native 1.28.0 DLLs from Microsoft's wheel and putting official CUDA 13.4 cuBLAS DLLs on `PATH` produced a successful CUDA decision and 1/5/10/50-question benchmark. Users of another ORT release must match its ABI and CUDA dependencies rather than copying arbitrary DLLs.
