import { SiteLayoutClient } from "./SiteLayoutClient";

export function generateStaticParams() {
  return [];
}

export default async function SiteLayout({
  children,
  params,
}: {
  children: React.ReactNode;
  params: Promise<{ id: string }> | { id: string };
}) {
  const resolved = params instanceof Promise ? await params : params;
  const id = Number((resolved as { id: string })?.id);
  return <SiteLayoutClient siteId={id}>{children}</SiteLayoutClient>;
}
