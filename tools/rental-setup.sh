#!/bin/bash
# One-time setup on a rented Linux GPU box (Vast.ai / RunPod pytorch image).
#
# Run from anywhere:   bash rental-setup.sh
# Assumes: CUDA-enabled PyTorch image (torch already installed), git access
# to the repo (private repos: set GIT_URL to an https URL with a token, e.g.
# https://<token>@github.com/carsonhu/awbw-ai.git).
#
# After it finishes, scp the checkpoints the grid needs into ~/awbw-ai/checkpoints/
# (bc-net2.pt at minimum; bc-powers-scaled2.pt and ppo-adder3.pt for panels),
# then run tools/grid.sh.
set -euo pipefail

GIT_URL="${GIT_URL:-https://github.com/carsonhu/awbw-ai.git}"

apt-get update -qq && apt-get install -y -qq build-essential curl git
if ! command -v cargo >/dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
fi

cd "$HOME"
[ -d awbw-ai ] || git clone --depth 1 "$GIT_URL"
cd awbw-ai
python3 -c "import torch; assert torch.cuda.is_available(), 'no CUDA'"
python3 -c "import numpy" 2>/dev/null || pip install -q numpy

# The Python extension is built against the box's own interpreter -- the
# checked-in awbw.pyd is a Windows artifact and does not travel.
PYO3_PYTHON="$(command -v python3)" cargo build --release -p awbw-py
cp target/release/libawbw.so python/awbw.so
python3 python/smoke_test.py

mkdir -p checkpoints logs
echo
echo "Setup complete. Now from the local machine:"
echo "  scp checkpoints/{bc-net2,bc-powers-scaled2,ppo-adder3}.pt <this-box>:awbw-ai/checkpoints/"
echo "then here:"
echo "  bash tools/grid.sh"
