# Contributing to Lowkey

First off, thank you for considering contributing to Lowkey! It's people like you that make Lowkey such a great tool.

## Where do I go from here?

If you've noticed a bug or have a feature request, [make one](https://github.com/your-username/lowkey/issues/new)!
It's generally best if you get confirmation of your bug or approval for your feature request this way before starting to code.

### Fork & create a branch

If this is something you think you can fix, then [fork Lowkey](https://github.com/your-username/lowkey/fork)
and create a branch with a descriptive name.

A good branch name would be (where issue #325 is the ticket you're working on):

```bash
git checkout -b 325-add-a-shiny-new-feature
```

### Get the code

```bash
git clone https://github.com/your-username/project.git
cd lowkey
```

### Run the tests

```bash
make test
```

### Implement your fix or feature

At this point, you're ready to make your changes! Feel free to ask for help; everyone is a beginner at first :smile_cat:

### Make a Pull Request

At this point, you should switch back to your master branch and make sure it's up to date with Lowkey's master branch:

```bash
git remote add upstream git@github.com:your-username/project.git
git checkout master
git pull upstream master
```

Then update your feature branch from your local copy of master, and push it!

```bash
git checkout 325-add-a-shiny-new-feature
git rebase master
git push --force-with-lease origin 325-add-a-shiny-new-feature
```

Finally, go to GitHub and [make a Pull Request](https://github.com/your-username/lowkey/compare/master...325-add-a-shiny-new-feature)
:D

## Guideline

*   Please follow the [Code of Conduct](docs/CODE_OF_CONDUCT.md).
*   Please make sure your code is formatted.
*   Please make sure your code is linted.
*   Please make sure your code is tested.

Thank you for your contribution!
