import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  transpilePackages: ['@tideline/arc', '@tideline/sdk', '@tideline/ui'],
};

export default nextConfig;
