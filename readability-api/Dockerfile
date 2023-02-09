FROM denoland/deno:1.29.1

WORKDIR /app
EXPOSE 3001
COPY ./deno.jsonc .
COPY ./deno.lock .
COPY ./main.ts .
RUN deno cache main.ts
CMD ["run", "--allow-net", "--allow-env", "--allow-read",  "main.ts"]
