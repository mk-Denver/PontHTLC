FROM node:22-bookworm-slim

WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .

RUN groupadd -r appuser && useradd -r -g appuser appuser
USER appuser

EXPOSE 5173
CMD ["npm", "run", "dev", "--", "--host", "0.0.0.0", "--port", "5173"]
