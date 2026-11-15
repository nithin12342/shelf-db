# setup.py - build the sheaf_torch PyTorch extension (spec Pillar 11).
#
# Install path (PyTorch is not installed yet):
#   1. Install PyTorch with libtorch (pip install torch).
#   2. python setup.py build_ext --inplace
#   3. python demo/train_snn.py
#
# The demo links a small stub arena (src/sheaf_handle_stub.c) so the extension
# builds standalone. In production the storage tier provides the real
# sheaf_handle_* symbols and the stub is dropped from the source list.

import os

from setuptools import setup
from torch.utils.cpp_extension import BuildExtension, CppExtension

HERE = os.path.dirname(os.path.abspath(__file__))

setup(
    name="sheaf_torch",
    version="0.1.0",
    description="Zero-copy BCSF stalk tensors and sheaf coboundary autograd",
    ext_modules=[
        CppExtension(
            name="sheaf_torch",
            sources=[
                os.path.join(HERE, "src", "sheaf_torch.cpp"),
                os.path.join(HERE, "src", "sheaf_handle_stub.c"),
            ],
            include_dirs=[os.path.join(HERE, "include")],
            extra_compile_args={"cxx": ["-O3", "-std=c++17"]},
        )
    ],
    cmdclass={"build_ext": BuildExtension},
)