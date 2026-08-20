import type { NextConfig } from "next";

const isWSLBuild = process.env.NEXT_PRIVATE_WSL_BUILD === "1";

const nextConfig: NextConfig = {
  // Skip static export under WSL bus-error; Windows host does `npm run build` with export.
  ...(isWSLBuild ? {} : { output: "export" }),
  trailingSlash: true,
  images: { unoptimized: true },

  async rewrites() {
    if (process.env.NODE_ENV !== "development") return [];
    const backend = process.env.NEXT_PUBLIC_API_PROXY || "http://127.0.0.1:8010";
    return [{ source: "/api/:path*", destination: `${backend}/api/:path*` }];
  },
};

export default nextConfig;
