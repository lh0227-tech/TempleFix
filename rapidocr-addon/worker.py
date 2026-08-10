"""TempleFix RapidOCR worker.

This process keeps the OCR models warm and exchanges one JSON object per line
over stdin/stdout. Images never leave the local machine.
"""

from __future__ import annotations

import argparse
import base64
import json
import sys
import time
from pathlib import Path

from rapidocr import RapidOCR


PROTOCOL_VERSION = 1
MAX_BASE64_CHARS = 32 * 1024 * 1024


def write_response(payload: dict) -> None:
    print(json.dumps(payload, ensure_ascii=False, separators=(",", ":")), flush=True)


def recognize(engine: RapidOCR, image_bytes: bytes) -> dict:
    started = time.perf_counter()
    result = engine(image_bytes)
    texts = tuple(result.txts or ())
    scores = tuple(result.scores or ())
    boxes = result.boxes
    lines = []
    for index, text in enumerate(texts):
        box = None
        if boxes is not None and index < len(boxes):
            box = [[round(float(x), 2), round(float(y), 2)] for x, y in boxes[index]]
        lines.append(
            {
                "text": str(text),
                "score": round(float(scores[index]), 6) if index < len(scores) else 0.0,
                "box": box,
            }
        )
    return {
        "ok": True,
        "protocol": PROTOCOL_VERSION,
        "elapsed_ms": round((time.perf_counter() - started) * 1000),
        "lines": lines,
    }


def make_engine() -> RapidOCR:
    return RapidOCR(
        params={
            "Global.log_level": "error",
            # Keep low-confidence lines in the worker response. TempleFix applies
            # its own threshold and can use the scores for quality routing.
            "Global.text_score": 0.0,
        }
    )


def serve() -> int:
    sys.stdin.reconfigure(encoding="utf-8")
    sys.stdout.reconfigure(encoding="utf-8", line_buffering=True)
    engine = make_engine()

    for raw_line in sys.stdin:
        request_id = None
        try:
            request = json.loads(raw_line)
            request_id = request.get("id")
            encoded = request.get("image_base64", "")
            if not isinstance(encoded, str) or not encoded:
                raise ValueError("缺少图片数据")
            if len(encoded) > MAX_BASE64_CHARS:
                raise ValueError("图片数据过大")
            image_bytes = base64.b64decode(encoded, validate=True)
            response = recognize(engine, image_bytes)
            response["id"] = request_id
            write_response(response)
        except Exception as exc:  # keep the worker alive for the next request
            write_response(
                {
                    "ok": False,
                    "protocol": PROTOCOL_VERSION,
                    "id": request_id,
                    "error": f"{type(exc).__name__}: {exc}",
                }
            )
    return 0


def self_test(image_path: Path) -> int:
    engine = make_engine()
    response = recognize(engine, image_path.read_bytes())
    write_response(response)
    return 0 if response["lines"] else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", type=Path)
    args = parser.parse_args()
    if args.self_test:
        return self_test(args.self_test)
    return serve()


if __name__ == "__main__":
    raise SystemExit(main())
