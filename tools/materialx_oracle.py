"""MaterialX's own reading of what this plugin writes: every document in
each WRITTEN folder read by MaterialX and validated, every node its
materials read resolved to a definition, and one the plugin wrote of an
example (the document of its name under EXAMPLES/*/) compared with
that example material by material: the nodes a document defines itself
flattened (MaterialX's flattenSubgraphs), every node a material reads,
through its graphs and their interfaces (MaterialX's traversal), by
category, definition and every input's value (its definition's where it
holds none), names aside. A local oracle, never a dependency: it needs
MaterialX 1.39.5's Python module, which tools/make_materialx.py installs.

    python3 tools/materialx_oracle.py tests/fixtures/examples tests/fixtures/written [WRITTEN...]

Prints one line a document and exits 1 when one doesn't validate, reads a
node of no definition or differs from its example, or a folder holds none.
"""
import pathlib
import sys

import MaterialX as mx


def read(path, libraries):
    doc = mx.createDocument()
    doc.importLibrary(libraries)
    search = mx.FileSearchPath(str(path.parent))
    mx.readFromXmlFile(doc, str(path), search)
    # A node the document defines itself as its graph, as the plugin reads
    # it; the library's nodes stay nodes.
    def own(node):
        definition = node.getNodeDef()
        return definition is not None and definition.getActiveSourceUri() == str(path)
    doc.flattenSubgraphs('', own)
    return doc


def number(text):
    """A value's text at MaterialX's float precision."""
    try:
        return tuple(round(float(x), 5) for x in text.split(','))
    except ValueError:
        return text


def value(port):
    """What an input holds, as compared: its value at MaterialX's float
    precision; a color's with its active color space, a file name resolved
    with its prefix and the color space of a color image."""
    text = port.getValueString()
    if port.getType() == 'filename':
        node = port.getParent()
        space = port.getActiveColorSpace() if node.getType() in ('color3', 'color4') else ''
        return (port.getResolvedValueString(), space)
    if port.getType() in ('color3', 'color4'):
        return (number(text), port.getActiveColorSpace() or 'lin_rec709')
    return number(text)


def nodes(material):
    """Every node `material` reads, by MaterialX's traversal, as a sorted
    list of (category, definition, every input its definition declares
    with what it holds: its value, else its definition's, or `wired`)."""
    seen = []
    for edge in material.traverseGraph():
        up = edge.getUpstreamElement()
        if up is None or not up.isA(mx.Node):
            continue
        definition = up.getNodeDef()
        if definition is None:
            seen.append((up.getCategory(), '?', ()))
            continue
        inputs = []
        for declared in definition.getActiveInputs():
            given = up.getInput(declared.getName())
            # An interface input holds what its graph's input holds.
            while given is not None and given.hasInterfaceName():
                given = up.getParent().getInput(given.getInterfaceName())
            if given is not None and (given.getConnectedNode() or given.hasNodeGraphString()):
                held = 'wired'
            elif declared.hasDefaultGeomPropString():
                # An input reading the geometry where nothing is wired: the
                # plugin leaves a value on it out, and says so, as Studio's
                # port takes a wire alone.
                held = None
            elif given is not None and given.hasValueString():
                held = value(given)
            elif declared.hasValueString():
                held = value(declared)
            else:
                held = None
            inputs.append((declared.getName(), held))
        seen.append((up.getCategory(), definition.getName(), tuple(inputs)))
    return sorted(set(seen), key=repr)


def materials(doc):
    found = {}
    for m in doc.getMaterialNodes():
        found[m.getAttribute('uiname') or m.getName()] = (nodes(m), m)
    return found


def main():
    examples, folders = pathlib.Path(sys.argv[1]), [pathlib.Path(a) for a in sys.argv[2:]]
    # The standard libraries the module carries.
    libraries = mx.createDocument()
    mx.loadLibraries(mx.getDefaultDataLibraryFolders(), mx.getDefaultDataSearchPath(), libraries)
    if not libraries.getNodeDefs():
        sys.exit(f'no MaterialX libraries in {mx.getDefaultDataSearchPath().asString()}')
    originals = {path.name: path for path in examples.glob('*/*.mtlx')}
    failed = 0
    for folder in folders:
        written = sorted(folder.glob('*.mtlx'))
        if not written:
            print(f'NONE in {folder}')
            failed += 1
        for path in written:
            doc = read(path, libraries)
            valid, why = doc.validate()
            theirs = materials(doc)
            unresolved = sorted({n[0] for seen, _ in theirs.values() for n in seen if n[1] == '?'})
            differ = []
            if path.name in originals:
                ours = materials(read(originals[path.name], libraries))
                for name, (seen, _) in ours.items():
                    other = theirs.get(name) or theirs.get(name.replace(' ', '_'))
                    if other is None or other[0] != seen:
                        differ.append(name)
            ok = valid and not unresolved and not differ
            failed += not ok
            print(f"{'ok' if ok else 'FAILED'} {path}: {len(theirs)} materials"
                  + ('' if path.name in originals else ', no example')
                  + ('' if valid else f'; invalid: {why.strip()}')
                  + (f'; no definition: {unresolved}' if unresolved else '')
                  + (f'; differ: {differ}' if differ else ''))
    sys.exit(1 if failed else 0)


if __name__ == '__main__':
    main()
