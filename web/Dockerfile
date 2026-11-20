# Zyvor Janus Web — Next.js dashboard
#
# Build (from repo root):
#   docker build -f web/Dockerfile \
#     --build-arg ZYVOR_JANUS_API_URL=http://zyvor-janus-api:8080 \
#     -t zyvor-janus-web:0.1.0 web
#
# Run:
#   docker run --rm -p 3000:3000 zyvor-janus-web:0.1.0

FROM node:20-bookworm-slim AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci

FROM node:20-bookworm-slim AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .

ARG ZYVOR_JANUS_API_URL=http://zyvor-janus-api:8080
ARG NEXT_PUBLIC_ZYVOR_JANUS_WS_URL=
ENV ZYVOR_JANUS_API_URL=${ZYVOR_JANUS_API_URL}
ENV NEXT_PUBLIC_ZYVOR_JANUS_WS_URL=${NEXT_PUBLIC_ZYVOR_JANUS_WS_URL}
ENV NEXT_TELEMETRY_DISABLED=1

RUN npm run build

FROM node:20-bookworm-slim AS runner
WORKDIR /app

ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1
ENV PORT=3000
ENV HOSTNAME=0.0.0.0

RUN useradd --create-home --shell /usr/sbin/nologin zyvor-janus

COPY --from=builder /app/public ./public
COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./.next/static

RUN chown -R zyvor-janus:zyvor-janus /app
USER zyvor-janus

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
  CMD node -e "fetch('http://127.0.0.1:3000/login').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"

CMD ["node", "server.js"]
