RELEASE=$1

if [ -z "$RELEASE" ]; then
    echo "usage: release [TAG | latest]"
    exit 1
fi

if [ "$RELEASE" = "latest" ]; then
    git fetch origin --tags --quiet
    LATEST=$(git tag --sort=-v:refname | head -n 1)
    if [ -z "$LATEST" ]; then
        echo "no tags found"
        exit 1
    fi
    echo "$LATEST"
    exit 0
fi

git tag "$RELEASE"
git push origin master "$RELEASE"
