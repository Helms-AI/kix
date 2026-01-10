# Runtime-only Dockerfile for web
# Build on host first: npm ci && npm run build

FROM nginx:alpine

# Copy pre-built files from host
COPY ./dist /usr/share/nginx/html

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
