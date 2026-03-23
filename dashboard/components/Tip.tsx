"use client";

import { useRef, useState, useCallback, useEffect } from "react";
import { createPortal } from "react-dom";

interface TipProps {
  text: string;
  inline?: boolean;
}

export default function Tip({ text, inline = false }: TipProps) {
  const [show, setShow] = useState(false);
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);
  const iconRef = useRef<HTMLSpanElement>(null);
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  const updatePos = useCallback(() => {
    if (!iconRef.current) return;
    const rect = iconRef.current.getBoundingClientRect();
    setPos({
      top: rect.top + window.scrollY,
      left: rect.left + rect.width / 2 + window.scrollX,
    });
  }, []);

  const handleEnter = () => {
    updatePos();
    setShow(true);
  };

  return (
    <>
      <span
        ref={iconRef}
        className={`${inline ? "ml-1" : ""} inline-block`}
        onMouseEnter={handleEnter}
        onMouseLeave={() => setShow(false)}
        onClick={() => {
          if (!show) updatePos();
          setShow(!show);
        }}
      >
        <span className="inline-flex items-center justify-center w-4 h-4 rounded-full bg-surface-600 text-text-secondary text-[10px] font-bold cursor-help hover:bg-accent/30 hover:text-accent-light transition-colors">
          ?
        </span>
      </span>
      {show && pos && mounted &&
        createPortal(
          <span
            style={{
              position: "absolute",
              top: pos.top - 8,
              left: pos.left,
              transform: "translate(-50%, -100%)",
              zIndex: 9999,
            }}
            className="w-64 px-3 py-2 text-xs text-text-primary bg-surface-800 border border-surface-600 rounded-lg shadow-lg leading-relaxed pointer-events-none"
          >
            {text}
            <span className="absolute top-full left-1/2 -translate-x-1/2 -mt-px border-4 border-transparent border-t-surface-800" />
            <span className="absolute top-full left-1/2 -translate-x-1/2 border-4 border-transparent border-t-surface-600" />
          </span>,
          document.body
        )}
    </>
  );
}
