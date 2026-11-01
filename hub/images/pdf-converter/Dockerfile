FROM python:3.11-slim
# LibreOffice (Writer のみ) + 日本語フォント
# Java 不要なので入れない (LibreOffice 7+ のヘッドレス変換は Java 不要)
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        libreoffice-writer \
        libreoffice-calc \
        libreoffice-impress \
        libreoffice-core \
        libreoffice-common \
        fonts-noto-cjk \
        fonts-noto-cjk-extra \
        fonts-ipafont-gothic \
        fonts-ipafont-mincho \
        fonts-takao \
        ca-certificates \
        curl \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean
# Python 依存
RUN pip install --no-cache-dir \
        fastapi==0.115.0 \
        uvicorn[standard]==0.32.0 \
        python-multipart==0.0.12 \
        openpyxl==3.1.5
# アプリ
WORKDIR /app
COPY pdf_converter.py /app/pdf_converter.py
# 非 root ユーザーで実行 (LibreOffice の権限問題回避)
RUN useradd -m -u 1000 appuser && \
    chown -R appuser:appuser /app
USER appuser
EXPOSE 8000
# シングルワーカー (LibreOffice の並列起動は競合の元)
# 複数並列で変換したい場合は Pod を replicas で増やす方針
CMD ["uvicorn", "pdf_converter:app", \
     "--host", "0.0.0.0", \
     "--port", "8000", \
     "--workers", "1", \
     "--log-level", "info"]

