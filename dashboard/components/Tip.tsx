"use client";

import { useState } from "react";

interface TipProps {
  /** Plain English explanation shown on hover/click */
  text: string;
  /** Optional: render inline next to a label instead of as a standalone icon */
  inline?: boolean;
}

/**
 * Tooltip component — shows a "?" icon that displays a plain English
 * explanation on hover (desktop) or tap (mobile).
 */
export default function Tip({ text, inline = false }: TipProps) {
  const [show, setShow] = useState(false);

  return (
    <span
      className={`relative ${inline ? "ml-1" : ""}`}
      onMouseEnter={() => setShow(true)}
      onMouseLeave={() => setShow(false)}
      onClick={() => setShow(!show)}
    >
      <span className="inline-flex items-center justify-center w-4 h-4 rounded-full bg-gray-200 text-gray-500 text-[10px] font-bold cursor-help hover:bg-blue-100 hover:text-blue-600 transition-colors">
        ?
      </span>
      {show && (
        <span className="absolute z-50 bottom-full left-1/2 -translate-x-1/2 mb-2 w-64 px-3 py-2 text-xs text-gray-700 bg-white border border-gray-200 rounded-lg shadow-lg leading-relaxed">
          {text}
          <span className="absolute top-full left-1/2 -translate-x-1/2 -mt-px border-4 border-transparent border-t-white" />
          <span className="absolute top-full left-1/2 -translate-x-1/2 border-4 border-transparent border-t-gray-200" />
        </span>
      )}
    </span>
  );
}
