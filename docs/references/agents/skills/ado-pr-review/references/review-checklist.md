# Checklist Revue PR ADO

## Avant conclusion

1. Ai-je lu la fiche PR complete ?
2. Ai-je lu le diff reel de la PR ?
3. Ai-je lu les fichiers complets des zones sensibles ?
4. Ai-je verifie les contrats/interfaces touches ?
5. Ai-je verifie le comportement autour des null/fallbacks ?
6. Ai-je verifie l'impact sur la persistance ?
7. Ai-je verifie l'impact sur la serialisation/API si un modele a change ?
8. Ai-je distingue clairement l'ancien comportement de la regression introduite ?

## Pour chaque finding

1. Le probleme est-il reellement introduit ou expose par cette PR ?
2. Puis-je citer un fichier et une ligne ?
3. Puis-je expliquer un scenario concret de casse ?
4. L'impact est-il compréhensible sans relire tout le diff ?
5. Est-ce un vrai risque produit/technique, pas juste une preference ?

## Si commentaire inline demande

1. Le commentaire est-il place sur la bonne ligne modifiee ?
2. Le commentaire commence-t-il par `[IA][github-copilot/gpt-5.4][...]` ?
3. Le texte est-il court, factuel, actionnable ?
4. Le commentaire evite-t-il les formulations vagues du type "attention" sans explication ?
5. Le commentaire explique-t-il l'impact ?
