import type { NextConfig } from "next";

const isDev = process.env.NODE_ENV === "development";

const nextConfig: NextConfig = {
  // Production: static export for Axum ServeDir
  ...(isDev ? {} : { output: "export" }),
  trailingSlash: true,
  images: { unoptimized: true },

  // Dev: proxy API to Rust backend
  async rewrites() {
    if (!isDev) return [];
    const backend = process.env.NEXT_PUBLIC_API_PROXY || "http://127.0.0.1:8010";
    return [
      {
        source: "/api/:path*",
        destination: `${backend}/api/:path*`,
      },
    ];
  },
};

export default nextConfig;
