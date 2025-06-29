use std::sync::Arc;

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};

// /// Validate a JWT token
// ///
// /// Return [`Claims`] if the token is valid
// pub fn validate_jwt(key: Arc<DecodingKey>, token: &str) -> AppResult<Claims> {
//     let mut validation = Validation::new(Algorithm::RS256);
//     validation.validate_exp = true;
//     validation.validate_nbf = true;

//     decode::<Claims>(token, key.as_ref(), &validation)
//         .map(|data| data.claims)
//         .map_err(|e| AppError::err(ErrType::Unauthorized, e, "Invalid JWT"))
// }
