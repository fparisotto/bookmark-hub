import { serve } from "https://deno.land/std@0.159.0/http/server.ts";
import { DOMParser } from "https://deno.land/x/deno_dom@v0.1.35-alpha/deno-dom-wasm.ts";
import { Readability } from "npm:@mozilla/readability";

async function handler(request: Request): Promise<Response> {
  if (request.method != "POST") {
    const body = JSON.stringify({
      message: "Wrong method, accept only post",
    });
    console.log("Wrong method, accept only post");
    return new Response(body, {
      status: 405,
      headers: {
        "content-type": "application/json; charset=utf-8",
        "Allow": "POST",
      },
    });
  }
  const body = await request.text();
  if (!body) {
    const body = JSON.stringify({
      message: "Bad request, send html content to be cleaned",
    });
    console.log("Empty body is bad request");
    return new Response(body, {
      status: 400,
      headers: {
        "content-type": "application/json; charset=utf-8",
      },
    });
  }
  const document = new DOMParser().parseFromString(body, "text/html");
  const reader = new Readability(document, { "debug": false });
  const article = reader.parse();
  const responseData = JSON.stringify(article);
  console.log(`Article processed title=${article?.title}`);
  return new Response(responseData, {
    status: 200,
    headers: { "content-type": "application/json; charset=utf-8" },
  });
}

serve(handler, { port: 3001 });
