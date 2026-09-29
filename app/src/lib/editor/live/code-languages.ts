// CodeMirror 5's mode list (MIT): a fence's name or alias in, its display name out.
const MODES =
	'APL;PGP;ASN.1;Asterisk;Brainfuck;C;C++|cpp;Cobol;C#|csharp|cs;Clojure;ClojureScript;Closure Stylesheets (GSS);CMake;CoffeeScript|coffee|coffee-script;Common Lisp|lisp;Cypher;Cython;Crystal;CSS;CQL;D;Dart;diff;Django;Dockerfile;DTD;Dylan;EBNF;ECL;edn;Eiffel;Elm;Embedded JavaScript;Embedded Ruby;Erlang;Esper;Factor;FCL;Forth;Fortran;F#|fsharp;Gas;Gherkin;GitHub Flavored Markdown;Go;Groovy;HAML;Haskell;Haskell (Literate);Haxe;HXML;ASP.NET|asp|aspx;HTML|xhtml;HTTP;IDL;Pug|jade;Java;Java Server Pages|jsp;JavaScript|ecmascript|js|node;JSON|json5;JSON-LD|jsonld;JSX;Jinja2;Julia|jl;Kotlin;LESS;LiveScript|ls;Lua;Markdown;mIRC;MariaDB SQL;Mathematica;Modelica;MUMPS;MS SQL;mbox;MySQL;Nginx;NSIS;NTriples;Objective-C|objective-c|objc;Objective-C++|objective-c++|objc++;OCaml;Octave;Oz;Pascal;PEG.js;Perl;PHP;Pig;Plain Text;PLSQL;PostgreSQL;PowerShell;Properties files|ini|properties;ProtoBuf;Python;Puppet;Q;R|rscript;reStructuredText|rst;RPM Changes;RPM Spec;Ruby|jruby|macruby|rake|rb|rbx;Rust;SAS;Sass;Scala;Scheme;SCSS;Shell|bash|sh|zsh;Sieve;Slim;Smalltalk;Smarty;Solr;SML;Soy|closure template;SPARQL|sparul;Spreadsheet|excel|formula;SQL;SQLite;Squirrel;Stylus;Swift;sTeX;LaTeX|tex;SystemVerilog;Tcl;Textile;TiddlyWiki;Tiki wiki;TOML;Tornado;troff;TTCN;TTCN_CFG;Turtle;TypeScript|ts;TypeScript-JSX|tsx;Twig;Web IDL;VB.NET;VBScript;Velocity;Verilog;VHDL;Vue.js Component;XML|rss|wsdl|xsd;XQuery;Yacas;YAML|yml;Z80;mscgen;xu;msgenny;WebAssembly';

const NAMES = new Map<string, string>();
for (const entry of MODES.split(';')) {
	const [name, ...aliases] = entry.split('|');
	for (const key of [name, ...aliases])
		if (!NAMES.has(key.toLowerCase())) NAMES.set(key.toLowerCase(), name);
}

export function languageLabel(language: string): string {
	return NAMES.get(language.toLowerCase()) ?? language;
}
