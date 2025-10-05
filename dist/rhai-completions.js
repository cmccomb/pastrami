const RHAI_KEYWORDS = [
    'if',
    'else',
    'switch',
    'match',
    'while',
    'loop',
    'for',
    'in',
    'let',
    'const',
    'mut',
    'fn',
    'private',
    'return',
    'break',
    'continue',
    'throw',
    'try',
    'catch',
    'import',
    'export',
    'true',
    'false',
    'null',
    'print',
    'debug',
    'type_of',
    'this',
    'is_shared',
    'shared',
    'eval'
];

const RHAI_NAMESPACES = ['rand', 'fs', 'url', 'ml', 'sci'];
const RHAI_NAMESPACE_COMPLETIONS = RHAI_NAMESPACES.reduce((accumulator, namespace) => {
    accumulator.push(namespace, `${namespace}::`);
    return accumulator;
}, []);
const FALLBACK_COMPLETIONS = [
    ...new Set([...RHAI_KEYWORDS, ...RHAI_NAMESPACE_COMPLETIONS])
].sort();

const tauriApi = window.__TAURI__ || {};
const invoke = typeof tauriApi.invoke === 'function' ? tauriApi.invoke.bind(tauriApi) : null;

let completionEntries = [...FALLBACK_COMPLETIONS];
let completionCatalogPromise = null;

async function ensureCompletionCatalogLoaded() {
    if (!completionCatalogPromise) {
        completionCatalogPromise = (async () => {
            if (!invoke) {
                return completionEntries;
            }

            try {
                const catalog = await invoke('rhai_completion_catalog');
                if (Array.isArray(catalog)) {
                    const sanitized = catalog.filter(
                        (entry) => typeof entry === 'string' && entry.trim().length > 0
                    );

                    if (sanitized.length > 0) {
                        completionEntries = [
                            ...new Set([...sanitized, ...RHAI_KEYWORDS])
                        ].sort();
                    }
                }
            } catch (error) {
                console.warn('Failed to load Rhai completion catalog.', error);
            }

            return completionEntries;
        })();
    }

    return completionCatalogPromise;
}

function matchesQualifiedCandidate(candidate, query) {
    if (!query) {
        return true;
    }

    if (candidate.startsWith(query)) {
        return true;
    }

    const separatorIndex = query.lastIndexOf('::');
    if (separatorIndex === -1) {
        return candidate.startsWith(query);
    }

    const namespacePrefix = query.slice(0, separatorIndex + 2);
    const lastSegmentPrefix = query.slice(separatorIndex + 2);

    if (!candidate.startsWith(namespacePrefix)) {
        return false;
    }

    if (!lastSegmentPrefix) {
        return true;
    }

    const remainder = candidate.slice(namespacePrefix.length);
    const nextSeparator = remainder.indexOf('::');
    const remainderSegment = nextSeparator === -1 ? remainder : remainder.slice(0, nextSeparator);
    return remainderSegment.startsWith(lastSegmentPrefix);
}

let showHintLoadPromise = null;

function ensureHintStylesheet() {
    const existing = document.querySelector('link[data-rhai-hint-css="true"]');
    if (existing) {
        return;
    }
    const link = document.createElement('link');
    link.rel = 'stylesheet';
    link.href = 'https://cdn.jsdelivr.net/npm/codemirror@5.65.16/addon/hint/show-hint.min.css';
    link.setAttribute('data-rhai-hint-css', 'true');
    document.head.appendChild(link);
}

function waitForCodeMirrorGlobal() {
    if (window.CodeMirror) {
        return Promise.resolve(window.CodeMirror);
    }

    return new Promise((resolve, reject) => {
        let attempts = 0;
        const maxAttempts = 200;
        const interval = window.setInterval(() => {
            attempts += 1;
            if (window.CodeMirror) {
                window.clearInterval(interval);
                resolve(window.CodeMirror);
            } else if (attempts > maxAttempts) {
                window.clearInterval(interval);
                reject(new Error('CodeMirror failed to load before timeout.'));
            }
        }, 25);
    });
}

async function ensureShowHintAssets() {
    ensureHintStylesheet();
    const codeMirror = await waitForCodeMirrorGlobal();

    if (codeMirror.showHint) {
        return codeMirror;
    }

    if (!showHintLoadPromise) {
        showHintLoadPromise = new Promise((resolve, reject) => {
            const script = document.createElement('script');
            script.src = 'https://cdn.jsdelivr.net/npm/codemirror@5.65.16/addon/hint/show-hint.min.js';
            script.async = true;
            script.onload = () => resolve();
            script.onerror = () => reject(new Error('Failed to load CodeMirror show-hint addon.'));
            document.head.appendChild(script);
        });
    }

    await showHintLoadPromise;
    return waitForCodeMirrorGlobal();
}

function registerRhaiHintHelper(codeMirror) {
    if (codeMirror.hint && codeMirror.hint.rhai) {
        return;
    }

    codeMirror.registerHelper('hint', 'rhai', (editor) => {
        const cursor = editor.getCursor();
        const token = editor.getTokenAt(cursor);
        const start = token.start;
        const end = cursor.ch;
        const prefix = token.string.slice(0, end - start);
        const normalizedPrefix = prefix.replace(/[^A-Za-z0-9_:.]/g, '');
        const matches = completionEntries.filter((candidate) =>
            matchesQualifiedCandidate(candidate, normalizedPrefix)
        );
        const list = matches.length > 0 ? matches : completionEntries;
        return {
            list,
            from: codeMirror.Pos(cursor.line, start),
            to: codeMirror.Pos(cursor.line, cursor.ch)
        };
    });
}

function mergeExtraKeys(editor, additionalKeys) {
    const existingKeys = editor.getOption('extraKeys') || {};
    editor.setOption('extraKeys', {...existingKeys, ...additionalKeys});
}

function shouldTriggerAutoComplete(changeEvent) {
    if (!changeEvent || changeEvent.origin !== '+input') {
        return false;
    }

    const text = (changeEvent.text || []).join('');
    if (!text || text.trim().length === 0) {
        return false;
    }

    return /^[A-Za-z0-9_.]$/.test(text);
}

export async function waitForCodeMirrorEditor(element) {
    await customElements.whenDefined('wc-codemirror');

    if (element.editor) {
        return element.editor;
    }

    return new Promise((resolve, reject) => {
        let attempts = 0;
        const maxAttempts = 200;
        const interval = window.setInterval(() => {
            attempts += 1;
            if (element.editor) {
                window.clearInterval(interval);
                resolve(element.editor);
            } else if (attempts > maxAttempts) {
                window.clearInterval(interval);
                reject(new Error('wc-codemirror editor did not initialize in time.'));
            }
        }, 25);
    });
}

export async function configureRhaiCompletions(editor) {
    if (!editor) {
        throw new Error('configureRhaiCompletions called without an editor instance.');
    }

    await ensureCompletionCatalogLoaded();
    const codeMirror = await ensureShowHintAssets();
    registerRhaiHintHelper(codeMirror);

    mergeExtraKeys(editor, {
        'Ctrl-Space': (cm) => {
            cm.showHint({hint: codeMirror.hint.rhai});
        }
    });

    editor.setOption('hintOptions', {
        hint: codeMirror.hint.rhai,
        completeSingle: false
    });

    editor.on('inputRead', (cm, change) => {
        if (shouldTriggerAutoComplete(change)) {
            cm.showHint({hint: codeMirror.hint.rhai});
        }
    });
}
