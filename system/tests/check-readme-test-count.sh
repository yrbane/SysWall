#!/bin/sh
# Verifie que les mentions du nombre de tests dans README.md correspondent au
# compte reel passe en argument. Evite la derive constatee a trois reprises
# (250 -> 356 en 0.3.5/0.3.6, puis 356 -> 358 en 0.3.7) faute de verification
# systematique au moment du commit.
#
# Verifies README.md test-count mentions match the real count passed as an
# argument. Prevents the drift observed three times in a row (250 -> 356 in
# 0.3.5/0.3.6, then 356 -> 358 in 0.3.7) for lack of a systematic check at
# commit time.
set -eu

ACTUAL="${1:?usage: check-readme-test-count.sh <actual_count> [readme_path]}"
README="${2:-README.md}"

if [ ! -f "$README" ]; then
    echo "ERROR: $README not found" >&2
    exit 1
fi

fail=0

check_mention() {
    # $1: motif ERE (grep -E), $2: description pour le message d'erreur
    n=$(grep -oE "$1" "$README" | head -1 | grep -oE '[0-9]+' || true)
    if [ -z "$n" ]; then
        echo "MISSING: $2 (motif introuvable dans $README)" >&2
        fail=$((fail + 1))
    elif [ "$n" != "$ACTUAL" ]; then
        echo "MISMATCH: $2 annonce $n, compte reel = $ACTUAL" >&2
        fail=$((fail + 1))
    fi
}

check_mention 'Tests-[0-9]+-brightgreen' "badge Tests"
check_mention '[0-9]+ tests unitaires et d.integration' "principe TDD"
check_mention 'Tous les tests \([0-9]+ tests\)' "commentaire du bloc cargo test"
check_mention '\| Tests \| \*\*[0-9]+\*\*' "tableau Statistiques"

if [ "$fail" -gt 0 ]; then
    echo "FAIL: $fail mention(s) de $README desynchronisee(s) du compte reel ($ACTUAL)" >&2
    exit 1
fi

echo "OK: toutes les mentions de $README correspondent au compte reel ($ACTUAL tests)"
