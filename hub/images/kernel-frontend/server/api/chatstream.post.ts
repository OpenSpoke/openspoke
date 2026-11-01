//  チャットの返答を流れてくるまま (SSE) 画面へ渡す中継。
//
//  /api/kernel はまとめて受け取ってから返すので、
//  書きかけの文字が少しずつ出てくる見え方にはできない。
//  この口だけは kernel の返答をそのまま素通しする。
//
//  使い方: POST /api/chatstream  { thread_id, content, model }

import { readBody, createError, setResponseHeaders, sendStream } from "h3";

export default defineEventHandler(async (event) => {
  const cfg = useRuntimeConfig();
  const body = await readBody<{
    thread_id?: string;
    content?: string;
    model?: string;
  }>(event);

  const threadId = (body?.thread_id || "").trim();
  const content = body?.content || "";
  if (!threadId || !content) {
    throw createError({
      statusCode: 400,
      statusMessage: "thread_id と content が必要です",
    });
  }

  const owner =
    String(event.node.req.headers["x-forwarded-preferred-username"] || "") ||
    String(event.node.req.headers["x-auth-request-email"] || "") ||
    "anonymous";

  const url = `${cfg.kernelUrl.replace(/\/$/, "")}/core/chat/send/stream`;

  const res = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Accept: "text/event-stream",
      "X-Forwarded-User": owner,
      "X-Forwarded-Email": String(
        event.node.req.headers["x-auth-request-email"] || "",
      ),
    },
    body: JSON.stringify({
      owner,
      thread_id: threadId,
      content,
      model: body?.model || "claude-sonnet-4-6",
    }),
  });

  if (!res.ok || !res.body) {
    const text = res.body ? await res.text() : "";
    throw createError({
      statusCode: res.status || 502,
      statusMessage: `kernel が受け付けません: ${text.slice(0, 200)}`,
    });
  }

  setResponseHeaders(event, {
    "Content-Type": "text/event-stream; charset=utf-8",
    "Cache-Control": "no-cache, no-transform",
    Connection: "keep-alive",
    //  途中で溜め込まれると少しずつ出てこないので、溜め込みを切る
    "X-Accel-Buffering": "no",
  });

  return sendStream(event, res.body as any);
});
