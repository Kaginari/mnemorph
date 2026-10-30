import type { ReactNode } from 'react';

/** An honest empty state: what is not here, and why, never a blank pane. */
export function Empty({
  glyph,
  title,
  children,
}: {
  glyph: string;
  title: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="empty" role="status">
      <div className="empty-glyph" aria-hidden="true">
        {glyph}
      </div>
      <h2 className="empty-title">{title}</h2>
      {children && <div className="empty-body">{children}</div>}
    </div>
  );
}
