import path from "node:path";
import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  allowedDevOrigins: ["127.0.0.1", "192.168.1.107"],
  outputFileTracingRoot: path.resolve(process.cwd()),
  serverExternalPackages: ["postgres"],
};

export default nextConfig;
