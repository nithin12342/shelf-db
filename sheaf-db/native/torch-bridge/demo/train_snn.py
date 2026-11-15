"""train_snn.py - sheaf neural network training demo (spec Pillar 11).

This demo exercises the sheaf_torch bridge. It minimizes the Dirichlet energy
||delta x||^2 on a small graph, which drives the 0-cochain x toward a global
section (delta x = 0). That harmonic solve is the core training step for a
sheaf neural network. The backward pass flows through the coboundary adjoint
delta* implemented in the C++ extension.

Run after building the extension:
    python setup.py build_ext --inplace
    python demo/train_snn.py

The demo uses only PyTorch, so it runs without PyTorch Geometric. A PyG model
can replace the manual graph with a Data object and call the same coboundary.
"""

import torch

import sheaf_torch


def line_graph(n):
    """Edge index tensors for a line graph on n vertices."""
    u = torch.arange(n - 1, dtype=torch.long)
    v = torch.arange(1, n, dtype=torch.long)
    return u, v


def check_adjoint():
    """Verify the backward pass is the true adjoint of the coboundary."""
    n = 6
    u, v = line_graph(n)
    x = torch.randn(n, dtype=torch.double, requires_grad=True)
    u = u.to(torch.long)
    v = v.to(torch.long)
    ok = torch.autograd.gradcheck(
        lambda t: sheaf_torch.coboundary(t, u, v), (x,), eps=1e-6, atol=1e-4
    )
    print("adjoint gradcheck:", "PASS" if ok else "FAIL")


def train():
    """Minimize Dirichlet energy with one anchored vertex."""
    torch.manual_seed(0)
    n = 8
    u, v = line_graph(n)
    x = torch.randn(n, requires_grad=True)
    opt = torch.optim.SGD([x], lr=0.1)
    for step in range(201):
        opt.zero_grad()
        energy = sheaf_torch.dirichlet_energy(x, u, v)
        anchor = (x[0] - 1.0) ** 2
        loss = energy + 10.0 * anchor
        loss.backward()
        opt.step()
        if step % 50 == 0:
            print(f"step {step:3d}  energy {energy.item():.6f}")
    print("final stalk:", [round(t, 4) for t in x.detach().tolist()])


if __name__ == "__main__":
    check_adjoint()
    train()