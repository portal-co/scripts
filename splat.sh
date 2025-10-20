D=$1
mkdir -p "$D/.github/workflows"
rm $D/.github/workflows/*
cp $(dirname $0)/actions/* "$D/.github/workflows"
cp $(dirname $0)/lint "$D/.github"

pushd "$D"
if [ -f "package.json" ]; then
npm install -D rolldown
fi
git add -A
git commit -m "Update"
git push