import { useEffect, useState } from "react";
import { Rss } from "lucide-react";

/** Favicon d'un flux (https://site/favicon.ico), ou l'icône RSS si le site n'en a pas. */
export default function Favicon({ siteUrl, size = 16 }: { siteUrl: string | null; size?: number }) {
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [siteUrl]);

  let src: string | null = null;
  try {
    if (siteUrl) src = `https://${new URL(siteUrl).host}/favicon.ico`;
  } catch {
    src = null;
  }

  if (!src || failed) return <Rss size={size} />;
  return (
    <img
      src={src}
      alt=""
      width={size}
      height={size}
      draggable={false}
      referrerPolicy="no-referrer"
      style={{ borderRadius: 3, objectFit: "contain" }}
      onError={() => setFailed(true)}
    />
  );
}
