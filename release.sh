RELEASE=$1

if [ -z "$RELEASE" ]; then
    echo "usage: release [TAG]"
    exit 1
fi

git tag "$RELEASE"
git push origin master --tags