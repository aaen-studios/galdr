import type { ReactNode } from "react";

/**
 * Lightweight markdown to React elements converter.
 * Handles common patterns found in GitHub release bodies:
 * headings, bold, inline code, code blocks, links, unordered lists.
 */
export function renderMarkdown(text: string): ReactNode[] {
  const blocks: ReactNode[] = [];
  const lines = text.split("\n");
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];

    // Code block (triple backticks)
    if (line.trimStart().startsWith("```")) {
      const lang = line.trimStart().slice(3).trim();
      const codeLines: string[] = [];
      i++;
      while (i < lines.length && !lines[i].trimStart().startsWith("```")) {
        codeLines.push(lines[i]);
        i++;
      }
      i++; // skip closing ```
      blocks.push(
        <pre key={`cb-${blocks.length}`} className="md-code-block">
          <code>{codeLines.join("\n")}</code>
        </pre>
      );
      continue;
    }

    // Heading (## or ###)
    const headingMatch = line.match(/^(#{2,3})\s+(.+)/);
    if (headingMatch) {
      const level = headingMatch[1]!.length;
      const content = inlineFormat(headingMatch[2]!);
      const Tag = level === 2 ? "h3" : "h4";
      blocks.push(
        <Tag key={`h-${blocks.length}`} className="md-heading">
          {content}
        </Tag>
      );
      i++;
      continue;
    }

    // Unordered list
    if (line.trimStart().match(/^[-*]\s+/)) {
      const items: ReactNode[] = [];
      while (i < lines.length && lines[i].trimStart().match(/^[-*]\s+/)) {
        const itemText = lines[i].trimStart().replace(/^[-*]\s+/, "");
        items.push(<li key={`li-${items.length}`}>{inlineFormat(itemText)}</li>);
        i++;
      }
      blocks.push(
        <ul key={`ul-${blocks.length}`} className="md-list">
          {items}
        </ul>
      );
      continue;
    }

    // Empty line — skip
    if (line.trim() === "") {
      i++;
      continue;
    }

    // Regular paragraph
    const paraLines: string[] = [line];
    i++;
    while (i < lines.length && lines[i].trim() !== "" && !lines[i].trimStart().startsWith("```") && !lines[i].trimStart().match(/^[-*]\s+/) && !lines[i].match(/^(#{2,3})\s+/)) {
      paraLines.push(lines[i]);
      i++;
    }
    blocks.push(
      <p key={`p-${blocks.length}`} className="md-paragraph">
        {inlineFormat(paraLines.join(" "))}
      </p>
    );
  }

  return blocks;
}

function inlineFormat(text: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  // Process inline patterns: code, bold, link
  const pattern = /(`[^`]+`)|(\*\*[^*]+\*\*)|(\[([^\]]+)\]\(([^)]+)\))/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  let key = 0;

  while ((match = pattern.exec(text)) !== null) {
    // Text before match
    if (match.index > lastIndex) {
      nodes.push(text.slice(lastIndex, match.index));
    }

    if (match[1]) {
      // Inline code
      nodes.push(
        <code key={`c-${key++}`} className="md-inline-code">
          {match[1].slice(1, -1)}
        </code>
      );
    } else if (match[2]) {
      // Bold
      nodes.push(
        <strong key={`b-${key++}`} className="md-bold">
          {match[2].slice(2, -2)}
        </strong>
      );
    } else if (match[3]) {
      // Link
      nodes.push(
        <a key={`a-${key++}`} href={match[5]!} className="md-link" target="_blank" rel="noopener noreferrer">
          {match[4]}
        </a>
      );
    }

    lastIndex = match.index + match[0].length;
  }

  // Remaining text
  if (lastIndex < text.length) {
    nodes.push(text.slice(lastIndex));
  }

  if (nodes.length === 0) return [text];
  return nodes;
}
