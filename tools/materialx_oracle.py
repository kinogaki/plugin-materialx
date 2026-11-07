"""MaterialX's own reading of what this plugin writes: each example document
(tests/fixtures/examples) and the document the plugin writes of it
(tests/fixtures/written) read by MaterialX, the written one validated, and
both compared material by material: the nodes a document defines itself
flattened (MaterialX's flattenSubgraphs), every node a material reads, through its
graphs and their interfaces (MaterialX's traversal), by category,
definition and every input's value (its definition's where it holds none),
names aside. A local oracle,
never a dependency: it needs MaterialX's Python module (1.39.5).

    python3 tools/materialx_oracle.py tests/fixtures/examples tests/fixtures/written

Prints one line a document and exits 1 when one differs or doesn't validate.
"""
import os
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
    examples, written = (pathlib.Path(a) for a in sys.argv[1:3])
    # The standard libraries beside the module, as its wheel lays them out.
    libraries = mx.createDocument()
    beside = mx.FileSearchPath(os.path.join(os.path.dirname(mx.__file__), 'materialx'))
    mx.loadLibraries(['libraries'], beside, libraries)
    failed = 0
    for original in sorted(examples.glob('*/*.mtlx')):
        copy = written / original.name
        a, b = read(original, libraries), read(copy, libraries)
        valid, why = b.validate()
        ours, theirs = materials(a), materials(b)
        differ = [name for name in ours if name not in theirs and name.replace(' ', '_') not in theirs]
        for name, (seen, _) in ours.items():
            other = theirs.get(name) or theirs.get(name.replace(' ', '_'))
            if other and other[0] != seen:
                differ.append(name)
        ok = valid and not differ
        failed += not ok
        print(f"{'ok' if ok else 'DIFFERS'} {original.name}: {len(ours)} materials"
              + ('' if valid else f'; invalid: {why.strip()}')
              + (f'; differ: {differ}' if differ else ''))
    sys.exit(1 if failed else 0)


if __name__ == '__main__':
    main()
