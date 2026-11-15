'use client';
import { useState } from 'react';

export default function CopyCommand({ command }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(command);
      setCopied(true);
      setTimeout(() => setCopied(false), 1600);
    } catch {
      /* clipboard blocked: the text is still selectable */
    }
  };
  return (
    <div className="copy-cmd">
      <code><span className="prompt" aria-hidden="true">$</span> {command}</code>
      <button type="button" onClick={copy} aria-live="polite">{copied ? 'Copied' : 'Copy'}</button>
    </div>
  );
}
