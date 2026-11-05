import type { MouseEvent, ReactNode } from 'react';
import { isExternalHttpUrl, openExternalUrl } from './openExternalUrl';

export interface ExternalLinkProps {
  /** Untrusted URL (metadata, notes, repo IDs). Only http/https becomes a link. */
  href: string;
  children: ReactNode;
  className?: string;
  title?: string;
  /** Keep the click from reaching an enclosing clickable row. */
  stopPropagation?: boolean;
}

/**
 * A link that opens in the system browser through the desktop bridge. It never
 * navigates the application window or opens a child window; anything that is
 * not an http(s) URL renders as plain text.
 */
export function ExternalLink({
  href,
  children,
  className,
  title,
  stopPropagation = false,
}: ExternalLinkProps) {
  if (!isExternalHttpUrl(href)) {
    return (
      <span className={className} title={title}>
        {children}
      </span>
    );
  }

  const handleClick = (event: MouseEvent<HTMLAnchorElement>) => {
    event.preventDefault();
    if (stopPropagation) {
      event.stopPropagation();
    }
    void openExternalUrl(href);
  };

  return (
    <a
      href={href}
      rel="noopener noreferrer"
      className={className}
      title={title}
      onClick={handleClick}
    >
      {children}
    </a>
  );
}
