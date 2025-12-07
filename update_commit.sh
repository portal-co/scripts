pushd "$1"
git add -A
git commit -m "Update"
git push
popd