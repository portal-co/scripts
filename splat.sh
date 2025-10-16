D=$1
mkdir -p "$D/.github/workflows"
rm $D/.github/workflows/*
cp $(dirname $0)/actions/* "$D/.github/workflows"
cp $(dirname $0)/lint "$d/.github"