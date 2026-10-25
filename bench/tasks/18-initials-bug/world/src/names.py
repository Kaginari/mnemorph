def initials(name):
    return ".".join(p[1].upper() for p in name.split())
