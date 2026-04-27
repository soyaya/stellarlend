# Stage 1: Build environment
FROM node:20-alpine AS builder
WORKDIR /app
# Copy package files and install all dependencies (including devDependencies for building)
COPY package*.json ./
RUN npm ci
# Copy the rest of the source code and build
COPY . .
RUN npm run build

# Stage 2: Production environment
FROM node:20-alpine AS runner
WORKDIR /app
# Copy package files and install ONLY production dependencies
COPY package*.json ./
RUN npm ci --omit=dev
# Copy the compiled output from the builder stage
COPY --from=builder /app/dist ./dist

EXPOSE 3000

# Start the application
CMD ["node", "dist/index.js"]