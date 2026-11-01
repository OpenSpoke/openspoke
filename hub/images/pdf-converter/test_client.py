"""動作確認用クライアント
ビルドしたイメージをローカルで起動して、変換できるかテストする。

使い方:
  python3 test_client.py path/to/sample.docx output.pdf
"""
import sys
import requests

if len(sys.argv) != 3:
    print("Usage: python3 test_client.py <input.docx> <output.pdf>")
    sys.exit(1)

input_path = sys.argv[1]
output_path = sys.argv[2]

with open(input_path, "rb") as f:
    files = {"file": (input_path, f, "application/vnd.openxmlformats-officedocument.wordprocessingml.document")}
    r = requests.post("http://localhost:8000/convert", files=files, timeout=120)

if r.status_code != 200:
    print(f"Failed: {r.status_code}")
    print(r.text[:500])
    sys.exit(1)

with open(output_path, "wb") as f:
    f.write(r.content)

print(f"OK: {len(r.content)} bytes saved to {output_path}")
