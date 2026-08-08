"""
Office → PDF / Office → DOCX 変換サービス (LibreOffice headless)

エンドポイント:
  GET  /health             ヘルスチェック
  POST /convert            Office ファイル (DOCX/XLSX/PPTX 等) を PDF に変換
  POST /convert-to-docx    旧 Word 形式 (.doc) / RTF / ODT 等を .docx に変換
                           (req_20260526_090257_9a89519b 対応)
"""
from fastapi import FastAPI, UploadFile, File, HTTPException
from fastapi.responses import Response
from urllib.parse import quote
import subprocess
import tempfile
import os
import logging
import time
import io

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(levelname)s - %(message)s"
)
logger = logging.getLogger("pdf_converter")

app = FastAPI(title="PDF Converter", version="1.1.0")

# LibreOffice の場所を起動時に確認
SOFFICE_BIN = None
for candidate in ["/usr/bin/soffice", "/usr/bin/libreoffice"]:
    if os.path.exists(candidate):
        SOFFICE_BIN = candidate
        break

if SOFFICE_BIN is None:
    logger.error("LibreOffice (soffice) not found. Service will not work.")
else:
    logger.info(f"LibreOffice found at: {SOFFICE_BIN}")


def _preprocess_xlsx_fit_to_width(xlsx_bytes: bytes) -> bytes:
    """XLSX ファイルの各シートに「横は1ページ、縦は何ページでも可」の印刷設定を埋め込む。

    LibreOffice は xlsx の page_setup を尊重するため、ここで openpyxl を使って
    page_setup.fitToWidth=1, page_setup.fitToHeight=0, fitToPage=True を全シートに設定する。
    これにより、PDF 変換後に「横が切れて次のページに溢れる」問題を防ぐ。

    失敗した場合は元バイト列をそのまま返す (フォールバック)。
    """
    try:
        from openpyxl import load_workbook
    except Exception as e:
        logger.warning(f"openpyxl import failed, skipping xlsx preprocessing: {e}")
        return xlsx_bytes

    try:
        wb = load_workbook(io.BytesIO(xlsx_bytes))
        modified = False
        for ws in wb.worksheets:
            # fitToWidth=1, fitToHeight=0 で「横は1ページ幅に収める、縦は何ページでも可」
            ws.page_setup.fitToWidth = 1
            ws.page_setup.fitToHeight = 0
            # fitToPage=True を設定しないと fitToWidth/fitToHeight が反映されない
            ws.sheet_properties.pageSetUpPr.fitToPage = True
            modified = True
        if not modified:
            return xlsx_bytes
        out = io.BytesIO()
        wb.save(out)
        return out.getvalue()
    except Exception as e:
        logger.warning(f"xlsx preprocessing failed, using original: {e}")
        return xlsx_bytes


@app.get("/health")
async def health():
    """ヘルスチェック"""
    if SOFFICE_BIN is None:
        return {"status": "unhealthy", "reason": "soffice not found"}
    return {"status": "ok", "soffice": SOFFICE_BIN}


@app.post("/convert")
async def convert(file: UploadFile = File(...)):
    """Office (DOCX/XLSX/PPTX 等) を PDF に変換する。

    リクエスト: multipart/form-data, file=Office ファイル
    レスポンス: application/pdf
    """
    if SOFFICE_BIN is None:
        raise HTTPException(status_code=500, detail="LibreOffice not available")

    # ファイル拡張子チェック (extension は緩く受け付ける)
    filename = file.filename or "input.docx"
    ext = os.path.splitext(filename)[1].lower()
    if ext not in [".docx", ".doc", ".odt", ".rtf",
                   ".xlsx", ".xls", ".ods",
                   ".pptx", ".ppt", ".odp"]:
        # 警告ログだけ出して続行
        logger.warning(f"Unusual file extension: {ext}, attempting conversion anyway")

    # 一時ディレクトリで作業
    with tempfile.TemporaryDirectory(prefix="pdfconv_") as tmpdir:
        in_path = os.path.join(tmpdir, "input" + (ext if ext else ".docx"))

        # 入力ファイル保存
        try:
            content = await file.read()
        except Exception as e:
            raise HTTPException(status_code=400, detail=f"failed to read upload: {e}")

        if not content:
            raise HTTPException(status_code=400, detail="empty file")

        # XLSX の場合は「横は1ページ幅に収める」前処理
        if ext == ".xlsx":
            preprocessed = _preprocess_xlsx_fit_to_width(content)
            if preprocessed is not content:
                logger.info(f"xlsx preprocessed: {len(content)} -> {len(preprocessed)} bytes (fitToWidth=1)")
                content = preprocessed

        with open(in_path, "wb") as f:
            f.write(content)

        logger.info(f"Converting: filename={filename} size={len(content)} bytes")

        # LibreOffice 変換実行
        # --headless: GUI 不要
        # --norestore: 前回起動状態を復元しない
        # --nofirststartwizard: 初回ウィザード抑止
        # --convert-to pdf: PDF 変換
        # --outdir: 出力先
        cmd = [
            SOFFICE_BIN,
            "--headless",
            "--norestore",
            "--nofirststartwizard",
            "--convert-to", "pdf",
            "--outdir", tmpdir,
            in_path,
        ]

        # ユーザー固有設定ディレクトリ (並列実行時の競合回避)
        env = os.environ.copy()
        userprofile = os.path.join(tmpdir, "lo_profile")
        env["HOME"] = tmpdir  # LibreOffice の設定を一時ディレクトリに作らせる

        t_start = time.time()
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                timeout=120,
                env=env,
            )
        except subprocess.TimeoutExpired:
            logger.error("LibreOffice timeout (>120s)")
            raise HTTPException(status_code=500, detail="conversion timeout")

        t_elapsed = time.time() - t_start

        if result.returncode != 0:
            stderr = result.stderr.decode("utf-8", errors="replace")[:1000]
            stdout = result.stdout.decode("utf-8", errors="replace")[:1000]
            logger.error(f"LibreOffice failed (rc={result.returncode}): stderr={stderr} stdout={stdout}")
            raise HTTPException(
                status_code=500,
                detail=f"conversion failed (rc={result.returncode}): {stderr[:300]}"
            )

        # 出力ファイル特定 (input.docx → input.pdf)
        in_basename = os.path.splitext(os.path.basename(in_path))[0]
        out_path = os.path.join(tmpdir, in_basename + ".pdf")

        if not os.path.exists(out_path):
            # フォールバック: tmpdir の .pdf を探す
            pdfs = [f for f in os.listdir(tmpdir) if f.lower().endswith(".pdf")]
            if pdfs:
                out_path = os.path.join(tmpdir, pdfs[0])
            else:
                logger.error(f"output PDF not found in {tmpdir}, files: {os.listdir(tmpdir)}")
                raise HTTPException(status_code=500, detail="output PDF not generated")

        with open(out_path, "rb") as f:
            pdf_bytes = f.read()

        logger.info(f"Conversion OK: input={len(content)} bytes -> output={len(pdf_bytes)} bytes in {t_elapsed:.2f}s")

        # ヘッダーで元ファイル名を返す (RFC 5987 形式: 日本語等のマルチバイト文字に対応)
        out_filename = os.path.splitext(filename)[0] + ".pdf"
        encoded_filename = quote(out_filename, safe="")
        return Response(
            content=pdf_bytes,
            media_type="application/pdf",
            headers={"Content-Disposition": f"inline; filename*=UTF-8''{encoded_filename}"}
        )


@app.post("/convert-to-docx")
async def convert_to_docx(file: UploadFile = File(...)):
    """旧 Word バイナリ形式 (.doc) / RTF / ODT 等を .docx に変換する。

    req_20260526_090257_9a89519b 対応。
    contract-backend が「先方提示の .doc を受け取りたい」 要件に応えるため、
    アップロード時にこのエンドポイントを呼んで .doc → .docx 変換してから
    既存の python-docx ベースの処理 (チェック / OUR_REVISION / Track Changes)
    に流す。

    LibreOffice の --convert-to docx 機能を使用。 内部で OOXML 形式に
    再構築されるため、 出力 .docx は python-docx で正しく開ける。

    リクエスト: multipart/form-data, file=Office ファイル (.doc 等)
    レスポンス: application/vnd.openxmlformats-officedocument.wordprocessingml.document
    """
    if SOFFICE_BIN is None:
        raise HTTPException(status_code=500, detail="LibreOffice not available")

    # 拡張子チェック (.docx を投げられても無害だが、 通常は変換不要なので警告)
    filename = file.filename or "input.doc"
    ext = os.path.splitext(filename)[1].lower()
    # docx に変換できる入力形式: .doc / .rtf / .odt / .txt / .docx(冪等)
    _allowed_in = [".doc", ".rtf", ".odt", ".txt", ".docx"]
    if ext not in _allowed_in:
        # 警告ログだけ出して続行 (LibreOffice が判定する)
        logger.warning(f"Unusual file extension for docx conversion: {ext}")

    with tempfile.TemporaryDirectory(prefix="docxconv_") as tmpdir:
        in_path = os.path.join(tmpdir, "input" + (ext if ext else ".doc"))

        try:
            content = await file.read()
        except Exception as e:
            raise HTTPException(status_code=400, detail=f"failed to read upload: {e}")

        if not content:
            raise HTTPException(status_code=400, detail="empty file")

        with open(in_path, "wb") as f:
            f.write(content)

        logger.info(f"Converting to docx: filename={filename} size={len(content)} bytes ext={ext}")

        # LibreOffice 変換実行 (docx ターゲット)
        # --convert-to docx:"Office Open XML Text" で .docx を出力。
        # Filter 指定なしの "docx" だけだと LibreOffice バージョンによっては
        # 古い .doc を出すことがあるため、 明示的にフィルタを指定する。
        cmd = [
            SOFFICE_BIN,
            "--headless",
            "--norestore",
            "--nofirststartwizard",
            "--convert-to", "docx:\"Office Open XML Text\"",
            "--outdir", tmpdir,
            in_path,
        ]

        env = os.environ.copy()
        env["HOME"] = tmpdir

        t_start = time.time()
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                timeout=120,
                env=env,
            )
        except subprocess.TimeoutExpired:
            logger.error("LibreOffice timeout (>120s)")
            raise HTTPException(status_code=500, detail="conversion timeout")

        t_elapsed = time.time() - t_start

        if result.returncode != 0:
            stderr = result.stderr.decode("utf-8", errors="replace")[:1000]
            stdout = result.stdout.decode("utf-8", errors="replace")[:1000]
            logger.error(f"LibreOffice (docx) failed (rc={result.returncode}): stderr={stderr} stdout={stdout}")
            raise HTTPException(
                status_code=500,
                detail=f"conversion failed (rc={result.returncode}): {stderr[:300]}"
            )

        # 出力ファイル特定 (input.doc → input.docx)
        in_basename = os.path.splitext(os.path.basename(in_path))[0]
        out_path = os.path.join(tmpdir, in_basename + ".docx")

        if not os.path.exists(out_path):
            # フォールバック: tmpdir 内の .docx を探す
            docxs = [f for f in os.listdir(tmpdir) if f.lower().endswith(".docx")]
            if docxs:
                out_path = os.path.join(tmpdir, docxs[0])
            else:
                logger.error(f"output DOCX not found in {tmpdir}, files: {os.listdir(tmpdir)}")
                raise HTTPException(status_code=500, detail="output DOCX not generated")

        with open(out_path, "rb") as f:
            docx_bytes = f.read()

        logger.info(
            f"DOCX conversion OK: input={len(content)} bytes -> output={len(docx_bytes)} bytes "
            f"in {t_elapsed:.2f}s"
        )

        # 出力ファイル名 (元の拡張子を .docx に置換)
        out_filename = os.path.splitext(filename)[0] + ".docx"
        encoded_filename = quote(out_filename, safe="")
        return Response(
            content=docx_bytes,
            media_type="application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            headers={"Content-Disposition": f"inline; filename*=UTF-8''{encoded_filename}"}
        )

