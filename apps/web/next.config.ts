import type { NextConfig } from "next";

// One .env at the repo root serves both the stream service and this app.
try {
  process.loadEnvFile("../../.env");
} catch {
  // No root .env: rely on the process environment (CI, hosting).
}

const nextConfig: NextConfig = {
  transpilePackages: ["@tideline/arc", "@tideline/schema", "@tideline/sdk", "@tideline/ui"],
};

export default nextConfig;
