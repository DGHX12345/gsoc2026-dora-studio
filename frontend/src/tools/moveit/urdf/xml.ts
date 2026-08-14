// Minimal XML parser for URDF (M13 D4) — attribute-only elements, no text
// nodes, no namespaces. URDF payloads are machine-generated and well-formed;
// this parser exists so URDF loading is testable in node (no DOMParser).

export interface XmlElement {
  name: string;
  attributes: Record<string, string>;
  children: XmlElement[];
}

const ENTITIES: Record<string, string> = {
  amp: '&',
  lt: '<',
  gt: '>',
  quot: '"',
  apos: "'",
};

export function parseXml(text: string): XmlElement {
  const parser = new XmlParser(text);
  parser.skipMisc();
  const root = parser.parseElement();
  parser.skipMisc();
  if (!parser.atEnd()) throw parser.error('trailing content after root element');
  return root;
}

class XmlParser {
  private pos = 0;

  constructor(private readonly text: string) {}

  atEnd(): boolean {
    return this.pos >= this.text.length;
  }

  error(message: string): Error {
    return new Error(`XML parse error at offset ${this.pos}: ${message}`);
  }

  /** Whitespace, comments and the prolog. */
  skipMisc() {
    for (;;) {
      this.skipWhitespace();
      if (this.text.startsWith('<?', this.pos)) {
        this.pos = this.text.indexOf('?>', this.pos);
        if (this.pos === -1) throw this.error('unterminated processing instruction');
        this.pos += 2;
      } else if (this.text.startsWith('<!--', this.pos)) {
        this.pos = this.text.indexOf('-->', this.pos);
        if (this.pos === -1) throw this.error('unterminated comment');
        this.pos += 3;
      } else if (this.text.startsWith('<!', this.pos)) {
        // DOCTYPE and friends: skip to the matching '>' (no nested brackets
        // in URDF declarations).
        this.pos = this.text.indexOf('>', this.pos);
        if (this.pos === -1) throw this.error('unterminated declaration');
        this.pos += 1;
      } else {
        return;
      }
    }
  }

  parseElement(): XmlElement {
    if (this.text[this.pos] !== '<') throw this.error('expected element');
    this.pos += 1;
    this.skipWhitespace();
    const name = this.readName();

    const attributes: Record<string, string> = {};
    for (;;) {
      this.skipWhitespace();
      const ch = this.text[this.pos];
      if (ch === '>' || ch === '/') break;
      if (ch === undefined) throw this.error('unterminated element');
      const attrName = this.readName();
      this.skipWhitespace();
      if (this.text[this.pos] !== '=') throw this.error(`expected '=' after attribute ${attrName}`);
      this.pos += 1;
      this.skipWhitespace();
      attributes[attrName] = this.readAttributeValue();
    }

    if (this.text[this.pos] === '/') {
      // Self-closing
      this.pos += 1;
      if (this.text[this.pos] !== '>') throw this.error('expected > after /');
      this.pos += 1;
      return { name, attributes, children: [] };
    }

    this.pos += 1; // consume '>'
    const children: XmlElement[] = [];
    for (;;) {
      this.skipMisc();
      if (this.text.startsWith('</', this.pos)) {
        this.pos += 2;
        this.skipWhitespace();
        const closing = this.readName();
        if (closing !== name) {
          throw this.error(`mismatched closing tag </${closing}> for <${name}>`);
        }
        this.skipWhitespace();
        if (this.text[this.pos] !== '>') throw this.error('expected > in closing tag');
        this.pos += 1;
        return { name, attributes, children };
      }
      if (this.atEnd()) throw this.error(`unclosed element <${name}>`);
      children.push(this.parseElement());
    }
  }

  private skipWhitespace() {
    while (this.pos < this.text.length && /\s/.test(this.text[this.pos])) {
      this.pos += 1;
    }
  }

  private readName(): string {
    const match = /^[A-Za-z_][A-Za-z0-9_.:-]*/.exec(this.text.slice(this.pos));
    if (!match) throw this.error('expected name');
    this.pos += match[0].length;
    return match[0];
  }

  private readAttributeValue(): string {
    const quote = this.text[this.pos];
    if (quote !== '"' && quote !== "'") throw this.error('expected quoted attribute value');
    this.pos += 1;
    let value = '';
    for (;;) {
      const ch = this.text[this.pos];
      if (ch === undefined) throw this.error('unterminated attribute value');
      if (ch === quote) {
        this.pos += 1;
        return unescapeEntities(value);
      }
      value += ch;
      this.pos += 1;
    }
  }
}

function unescapeEntities(value: string): string {
  return value.replace(/&([A-Za-z]+|#[0-9]+|#x[0-9a-fA-F]+);/g, (entity, body: string) => {
    if (body.startsWith('#')) {
      const code = body.startsWith('#x')
        ? parseInt(body.slice(2), 16)
        : parseInt(body.slice(1), 10);
      return String.fromCodePoint(code);
    }
    const decoded = ENTITIES[body];
    if (decoded === undefined) throw new Error(`XML parse error: unknown entity &${body};`);
    return decoded;
  });
}
